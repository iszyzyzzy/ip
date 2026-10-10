import copy
from concurrent import futures
import json
from pathlib import Path
import struct
import tempfile
import time
import unittest
from unittest.mock import patch

import grpc
from google.protobuf.json_format import ParseDict
from h7.tools.dm_can import Bridge, Packet, ACK, RX, TX_RESULT, TX, INFO, MOTOR_STATUS
from robotd.config import default_config, validate_config
from robotd.control import Controller, ControlError
from robotd.drivers import Hardware, ServoBus, Simulated, chassis_payload, decode_chassis
from robotd.pb import robot_pb2 as pb, robot_pb2_grpc as pg
from robotd.pb import eyes_pb2 as ep, eyes_pb2_grpc as eg
from robotd.server import Service, InstanceLock
from robotd.store import Store, Conflict
from robotd.validation import validate_targets


class ServoSerial:
    def __init__(self):
        self.events = []
        self.buffer = bytearray()
        self.responses = []
        self.closed = False

    @property
    def in_waiting(self):
        return len(self.buffer)

    @property
    def rts(self):
        return self._rts

    @rts.setter
    def rts(self, value):
        self.events.append(("rts", value))
        self._rts = value

    def write(self, wire):
        self.events.append(("write", wire))
        if wire.endswith(b"PRAD!"):
            self.buffer.extend(wire + wire[:5] + b"1500!")
        return len(wire)

    def flush(self):
        self.events.append(("drain",))

    def reset_input_buffer(self):
        self.buffer.clear()

    def read(self, size):
        data = bytes(self.buffer[:size])
        del self.buffer[:size]
        return data

    def close(self):
        self.closed = True


class H7Serial:
    def __init__(self):
        self.buffer = bytearray()
        self.packets = []
        self.closed = False

    @property
    def in_waiting(self):
        return len(self.buffer)

    def write(self, wire):
        if wire == b"\0":
            return 1
        p = Packet.decode(wire[:-1])
        self.packets.append(p)
        extra = b""
        if p.type == INFO:
            extra = bytes([1, 1, 0, 3, 64, 16, 0, 0]) + b"DM-H723"
        elif p.type == MOTOR_STATUS:
            extra = b"".join(struct.pack("<BBBBfffffII", i, 0, 0, 30, 0, 0, 0, 0, 0, 5, 0)
                             for i in (1, 2)) + bytes(4)
        if p.type == TX:
            can_id = struct.unpack_from("<I", p.payload)[0]
            data = p.payload[5:]
            if can_id == 0x141:
                self.rx(0x291, bytes([data[0], 0, 0]))
            if can_id == 0x421:
                self.rx(0x211, struct.pack(">BBHI", 0, 1, 240, 0))
            self.buffer.extend(Packet(TX_RESULT, p.channel, sequence=p.sequence,
                                      payload=bytes(5)).encode())
        self.buffer.extend(Packet(ACK, p.channel, sequence=p.sequence,
                                  payload=bytes([p.type, 0]) + extra).encode())
        return len(wire)

    def rx(self, can_id, data, channel=2):
        self.buffer.extend(Packet(RX, channel, payload=struct.pack("<IIB", can_id, 10, len(data))
                                  + data).encode())

    def read(self, size):
        # 验证真实 Bridge 解析拆分后的 CDC 报文，而不是直接伪造 API 返回值。
        size = min(size, 7)
        data = bytes(self.buffer[:size])
        del self.buffer[:size]
        return data

    def close(self):
        self.closed = True


class ProtocolTests(unittest.TestCase):
    def test_ranger_document_vectors_and_feedback(self):
        self.assertEqual(chassis_payload(pb.ChassisTarget(speed_m_s=0.15)).hex(),
                         "0096000000000000")
        self.assertEqual(chassis_payload(pb.ChassisTarget(steering_rad=1)).hex(),
                         "00000000000003e8")
        key, value = decode_chassis(0x211, bytes.fromhex("000100f000000080"))
        self.assertEqual((key, value["voltage_v"], value["fault_bits"]), ("system", 24, 128))
        self.assertIsNone(decode_chassis(0x211, b"\0"))
        self.assertEqual(decode_chassis(0x221, bytes.fromhex("ff6aff3800000064"))[1],
            dict(speed_m_s=-0.15, yaw_rate_rad_s=-0.2, steering_raw_rad=0.1))

    def test_rts_drain_release_and_echo(self):
        cfg = default_config()["left_arm"]
        transport = ServoSerial()
        bus = ServoBus(cfg, transport)
        transport.events.clear()
        bus.position(pb.ArmTarget(joint_rad=[0, 0, 0, 0], duration_ms=500))
        self.assertEqual(transport.events[:4], [("rts", True),
            ("write", b"#000P1500T0500!"), ("drain",), ("rts", False)])
        self.assertEqual(bus.read_position(0)["position_rad"], 0)
        transport.write = lambda _: (_ for _ in ()).throw(IOError("写入失败"))
        with self.assertRaises(IOError):
            bus.send(b"#000PDST!")
        self.assertFalse(transport.rts)
        bus.close()
        self.assertTrue(transport.closed)

    def test_h7_real_bridge_and_mode_feedback_gate(self):
        cfg = default_config()
        cfg["eyes"]["enabled"] = False
        cfg["left_arm"]["enabled"] = cfg["right_arm"]["enabled"] = False
        transport = H7Serial()
        bridge = Bridge("test", transport=transport, timeout=0.1)
        with patch("robotd.drivers.Bridge", return_value=bridge), \
                patch("robotd.drivers.serial.Serial", return_value=transport):
            hardware = Hardware(cfg)
            hardware.connect()
            target = pb.ChassisTarget(mode=pb.ACKERMANN, speed_m_s=0.15, ttl_ms=300)
            now = time.monotonic()
            hardware.chassis(target, now)
            self.assertFalse(hardware.state["chassis"]["ready"])
            hardware.poll(now + 0.01)
            hardware.chassis(target, now + 0.02)
            self.assertTrue(hardware.state["chassis"]["ready"])
            motion = [p for p in transport.packets if p.type == TX
                      and struct.unpack_from("<I", p.payload)[0] == 0x111]
            self.assertEqual(motion[-1].payload[5:], bytes.fromhex("0096000000000000"))
            hardware.chassis(target, now + 1)
            self.assertFalse(hardware.state["chassis"]["ready"])
            self.assertEqual(transport.packets[-1].payload[5:], bytes(8))
            hardware.close()
            self.assertTrue(transport.closed)

    def test_whole_command_validation_and_limits(self):
        cfg = default_config()
        cases = [pb.Targets(neck=pb.NeckTarget(yaw_rad=float("nan"))),
                 pb.Targets(chassis=pb.ChassisTarget(mode=pb.SPIN, speed_m_s=0.1, ttl_ms=300)),
                 pb.Targets(left_arm=pb.ArmTarget(joint_rad=[0, 0, 0], duration_ms=500)),
                 pb.Targets(neck=pb.NeckTarget(yaw_rad=0.8)), pb.Targets()]
        for target in cases:
            with self.subTest(target=target), self.assertRaises(ValueError):
                validate_targets(target, cfg)
        cfg["chassis"]["max_speed_m_s"] = 1.5
        with self.assertRaises(ValueError):
            validate_targets(pb.Targets(chassis=pb.ChassisTarget(
                speed_m_s=0.6, steering_rad=0.4, ttl_ms=300)), cfg)
        validate_targets(pb.Targets(neck=pb.NeckTarget(yaw_rad=0)), cfg)


class StoreTests(unittest.TestCase):
    def test_persistence_conflict_and_invalid_config(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[2]) as tmp:
            path = Path(tmp) / "robot.sqlite"
            store = Store(path)
            rev, cfg = store.config()
            cfg["metadata"]["name"] = "机器人"
            self.assertEqual(store.update_config(rev, cfg)[0], 2)
            with self.assertRaises(Conflict):
                store.update_config(rev, cfg)
            bad = copy.deepcopy(cfg)
            bad["left_arm"]["joints"][1]["id"] = 0
            with self.assertRaises(ValueError):
                store.update_config(2, bad)
            bad = copy.deepcopy(cfg)
            bad["eyes"]["settings"]["jelly"]["frequency"] = 100
            with self.assertRaises(ValueError):
                validate_config(bad)
            store.close()
            store = Store(path)
            self.assertEqual(store.config(), (2, cfg))
            store.close()

    def test_action_delete_keeps_revision_and_instance_lock(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[2]) as tmp:
            path = Path(tmp) / "robot.sqlite"
            store = Store(path)
            value = {"name": "test"}
            self.assertEqual(store.put_action("test", 0, value), 1)
            self.assertEqual(store.delete_action("test", 1), 2)
            with self.assertRaises(Conflict):
                store.put_action("test", 0, value)
            self.assertEqual(store.put_action("test", 2, value), 3)
            store.close()
            lock = InstanceLock(path)
            try:
                with self.assertRaises(RuntimeError):
                    InstanceLock(path)
            finally:
                lock.close()
            InstanceLock(path).close()


class ControlTests(unittest.TestCase):
    def setUp(self):
        self.now = 100.0
        self.store = Store(":memory:")
        self.control = Controller(self.store, simulated=True, clock=lambda: self.now)
        self.control.reconnect()

    def tearDown(self):
        self.store.close()

    def lease(self, ttl=1000):
        return self.control.acquire(pb.AcquireRequest(owner="test", ttl_ms=ttl))

    def test_exclusive_lease_and_independent_chassis_ttl(self):
        lease = self.lease()
        with self.assertRaises(ControlError):
            self.lease()
        self.control.set_targets(pb.TargetRequest(token=lease.token,
            targets=pb.Targets(chassis=pb.ChassisTarget(speed_m_s=0.1, ttl_ms=100))))
        self.now += 0.11
        self.control.renew(pb.LeaseRequest(token=lease.token))
        self.control.tick()
        self.assertIsNone(self.control.chassis_target)
        self.assertIsNotNone(self.control.lease)
        self.assertEqual(self.control.driver.state["events"][-1]["operation"], "stop_chassis")
        self.now += 1.01
        self.control.tick()
        self.assertIsNone(self.control.lease)
        self.assertTrue(self.control.stopped)

    def test_latched_stop_and_stale_token(self):
        lease = self.lease()
        self.control.stop(pb.StopRequest(latch=True))
        with self.assertRaises(ControlError):
            self.control.set_targets(pb.TargetRequest(token=lease.token,
                targets=pb.Targets(neck=pb.NeckTarget(yaw_rad=0))))
        with self.assertRaises(ControlError):
            self.lease()
        new = self.control.acquire(pb.AcquireRequest(owner="new", ttl_ms=1000, clear_latch=True))
        self.assertNotEqual(new.token, lease.token)

    def test_action_timing_cancel_and_validation_before_io(self):
        action = pb.Action(name="test", duration_ms=1000, steps=[
            pb.ActionStep(at_ms=0, targets=pb.Targets(neck=pb.NeckTarget(yaw_rad=0.1))),
            pb.ActionStep(at_ms=500, targets=pb.Targets(neck=pb.NeckTarget(pitch_rad=0.1)))])
        self.control.put_action(pb.ActionUpdate(action=action))
        lease = self.lease(ttl=3000)
        self.control.run_action(pb.RunRequest(token=lease.token, name="test"))
        self.control.tick()
        self.assertEqual(self.control.action_index, 1)
        self.now += 0.5
        self.control.tick()
        self.assertEqual(self.control.action_index, 2)
        self.control.cancel_action(pb.LeaseRequest(token=lease.token))
        count = len(self.control.driver.state["events"])
        self.now += 0.1
        self.control.tick()
        self.assertEqual(len(self.control.driver.state["events"]), count)
        with self.assertRaises(ValueError):
            self.control.set_targets(pb.TargetRequest(token=lease.token, targets=pb.Targets(
                neck=pb.NeckTarget(yaw_rad=0.1), left_arm=pb.ArmTarget(joint_rad=[10]*4))))
        self.assertEqual(len(self.control.driver.state["events"]), count)

    def test_config_apply_failure_is_persisted_and_revokes_lease(self):
        self.lease()
        class Failing(Simulated):
            def connect(self):
                raise IOError("设备不存在")
        self.control.driver_factory = Failing
        snapshot = self.control.get_config()
        updated = self.control.update_config(pb.ConfigUpdate(expected_revision=1,
                                                               config=snapshot.config))
        self.assertEqual(updated.revision, 2)
        self.assertEqual(self.store.config()[0], 2)
        self.assertEqual(self.control.applied_revision, 0)
        self.assertIsNone(self.control.lease)
        with self.assertRaises(ControlError):
            self.lease()

    def test_action_completion_and_late_schedule_stop(self):
        self.lease(ttl=3000)
        self.control.action = pb.Action(name="late", duration_ms=1000, steps=[
            pb.ActionStep(at_ms=0, targets=pb.Targets(neck=pb.NeckTarget(yaw_rad=0)))])
        self.control.action_start = self.now
        self.now += 0.3
        self.control.tick()
        self.assertTrue(self.control.fault)
        self.assertTrue(self.control.stopped)
        self.assertIsNone(self.control.lease)


class EyeService(eg.EyeControlServicer):
    def __init__(self):
        self.state = ep.State()

    def Configure(self, request, context):
        self.state.config.CopyFrom(request)
        self.state.revision += 1
        return ep.Ack(revision=self.state.revision)

    def SetPose(self, request, context):
        self.state.rendered_pose.CopyFrom(request)
        self.state.config.mode = ep.EXTERNAL
        self.state.revision += 1
        return ep.Ack(revision=self.state.revision)

    def GetState(self, request, context):
        return self.state


class RpcTests(unittest.TestCase):
    def test_real_grpc_config_stream_stop_and_actions(self):
        store = Store(":memory:")
        controller = Controller(store, simulated=True)
        controller.start()
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=6))
        pg.add_RobotControlServicer_to_server(Service(controller), server)
        port = server.add_insecure_port("127.0.0.1:0")
        server.start()
        try:
            with grpc.insecure_channel(f"127.0.0.1:{port}",
                                       options=[("grpc.enable_http_proxy", 0)]) as channel:
                grpc.channel_ready_future(channel).result(timeout=3)
                stub = pg.RobotControlStub(channel)
                cfg = stub.GetConfig(pb.Empty(), timeout=3)
                cfg.config["metadata"]["name"] = "grpc-test"
                result = stub.UpdateConfig(pb.ConfigUpdate(expected_revision=cfg.revision,
                                                            config=cfg.config), timeout=3)
                self.assertEqual(result.revision, 2)
                with self.assertRaises(grpc.RpcError) as conflict:
                    stub.UpdateConfig(pb.ConfigUpdate(expected_revision=1, config=cfg.config), timeout=3)
                self.assertEqual(conflict.exception.code(), grpc.StatusCode.ABORTED)
                lease = stub.AcquireControl(pb.AcquireRequest(owner="grpc", ttl_ms=1000), timeout=3)
                stream = [pb.TargetRequest(token=lease.token,
                           targets=pb.Targets(neck=pb.NeckTarget(yaw_rad=v))) for v in (0, 0.1)]
                stub.StreamTargets(iter(stream), timeout=3)
                watch = stub.WatchState(pb.WatchRequest(interval_ms=50), timeout=3)
                self.assertEqual(next(watch).owner, "grpc")
                watch.cancel()
                action = ParseDict(json.loads(Path("robotd/examples/greeting.json").read_text()), pb.Action())
                stub.PutAction(pb.ActionUpdate(action=action), timeout=3)
                self.assertEqual(len(stub.ListActions(pb.Empty(), timeout=3).actions), 1)
                stub.RunAction(pb.RunRequest(token=lease.token, name="greeting"), timeout=3)
                stub.CancelAction(pb.LeaseRequest(token=lease.token), timeout=3)
                stub.DeleteAction(pb.ActionDelete(name="greeting", expected_revision=1), timeout=3)
                deleted = stub.ListActions(pb.Empty(), timeout=3).actions[0]
                self.assertTrue(deleted.deleted)
                self.assertEqual(deleted.revision, 2)
                stub.PutAction(pb.ActionUpdate(action=action, expected_revision=2), timeout=3)
                stub.Stop(pb.StopRequest(latch=True), timeout=3)
                state = stub.GetState(pb.Empty(), timeout=3)
                self.assertTrue(state.latched and state.stopped)
                self.assertFalse(state.owner)
                self.assertTrue(state.simulated)
        finally:
            server.stop(0).wait(3)
            controller.close()

    def test_actual_eye_rpc_config_pose_and_authority(self):
        eye = EyeService()
        server = grpc.server(futures.ThreadPoolExecutor(max_workers=2))
        eg.add_EyeControlServicer_to_server(eye, server)
        port = server.add_insecure_port("127.0.0.1:0")
        server.start()
        cfg = default_config()
        for device in ("chassis", "neck", "left_arm", "right_arm"):
            cfg[device]["enabled"] = False
        cfg["eyes"]["target"] = f"127.0.0.1:{port}"
        driver = Hardware(cfg)
        try:
            driver.connect()
            targets = pb.Targets(eyes=ep.Pose(left=ep.Eye(openness=1, gaze_x=0.1),
                                             right=ep.Eye(openness=1)))
            driver.apply(targets)
            self.assertEqual(eye.state.config.mode, ep.EXTERNAL)
            eye.state.config.show_controls = True
            driver.poll(time.monotonic() + 1)
            self.assertFalse(eye.state.config.show_controls)
            self.assertAlmostEqual(eye.state.rendered_pose.left.gaze_x, 0.1)
        finally:
            driver.close()
            server.stop(0).wait(3)


if __name__ == "__main__":
    unittest.main()
