"""设备协议与传输；仅由控制线程访问，H7 CDC 会话保持打开。"""
from __future__ import annotations

import copy
import os
import re
import struct
import time
import grpc
import serial
from google.protobuf.json_format import MessageToDict, ParseDict
from h7.tools.dm_can import Bridge
from .pb import eyes_pb2 as ep, eyes_pb2_grpc as eg


def chassis_payload(target):
    # RANGER AIR 的保留字节为零，没有 Scout V1 的计数或校验字段。
    return struct.pack(">hhhh", round(target.speed_m_s * 1000),
                       round(target.yaw_rate_rad_s * 1000), 0,
                       round(target.steering_rad * 1000))


def decode_chassis(can_id, data):
    if can_id == 0x211 and len(data) == 8:
        state, control, voltage, fault = struct.unpack(">BBHI", data)
        return "system", dict(state=state, control_mode=control, voltage_v=voltage / 10,
                              fault_bits=fault)
    if can_id == 0x221 and len(data) == 8:
        speed, yaw, _, angle = struct.unpack(">hhhh", data)
        return "motion", dict(speed_m_s=speed / 1000, yaw_rate_rad_s=yaw / 1000,
                              steering_raw_rad=angle / 1000)
    if can_id == 0x291 and len(data) == 3:
        return "mode", dict(mode=data[0], switching=data[1] != 0, drive_mode=data[2])
    if can_id in (0x271, 0x281) and len(data) == 8:
        return ("wheel_angles" if can_id == 0x271 else "wheel_speeds"), {
            "values": [v / 1000 for v in struct.unpack(">hhhh", data)]}
    if can_id in (0x311, 0x312) and len(data) == 8:
        left, right = struct.unpack(">ii", data)
        return ("front_odometry" if can_id == 0x311 else "rear_odometry"), {
            "left_m": left / 1000, "right_m": right / 1000}
    if can_id == 0x241 and len(data) == 8:
        switches, *axes = struct.unpack("B5b2x", data)
        return "remote", dict(swa=switches & 3, swb=(switches >> 2) & 3,
                              swc=(switches >> 4) & 3, swd=(switches >> 6) & 3,
                              axes=axes)
    if can_id == 0x361 and len(data) == 8:
        soc, soh, voltage, current, temperature = struct.unpack(">BBHhh", data)
        return "battery", dict(soc=soc, soh=soh, voltage_v=voltage / 10,
                               current_a=current / 10, temperature_c=temperature / 10)
    if can_id == 0x362 and len(data) == 4:
        return "battery_alarm", dict(alarm=list(data[:2]), warning=list(data[2:]))
    if 0x251 <= can_id <= 0x258 and len(data) == 8:
        rpm, current, position = struct.unpack(">hhi", data)
        return f"motor_{can_id - 0x250}_fast", dict(rpm=rpm, current_a=current / 10,
                                                  position_counts=position)
    if 0x261 <= can_id <= 0x268 and len(data) == 8:
        voltage, temperature, motor_temp, status = struct.unpack(">HhbB2x", data)
        return f"motor_{can_id - 0x260}_slow", dict(voltage_v=voltage / 10,
                                                  driver_temperature_c=temperature,
                                                  motor_temperature_c=motor_temp, status=status)
    return None


def arm_pwm(joint, angle):
    return round(joint["center_pwm"] + joint["direction"] * joint["pwm_per_rad"] * angle)


class ServoBus:
    def __init__(self, cfg, transport=None):
        self.cfg = cfg
        self.kernel = cfg["direction_mode"] == "kernel"
        if transport is None:
            self.serial = serial.Serial(port=None, baudrate=int(cfg["baudrate"]),
                                        timeout=0.005, write_timeout=0.2,
                                        rtscts=False, dsrdtr=False,
                                        exclusive=True if os.name == "posix" else None)
            # 先设接收状态再打开设备，避免打开时主动驱动总线。
            self.serial.rts = not cfg["rts_tx"]
            self.serial.port = cfg["port"]
            self.serial.open()
        else:
            self.serial = transport
        try:
            if self.kernel:
                from serial.rs485 import RS485Settings
                self.serial.rs485_mode = RS485Settings(
                    rts_level_for_tx=cfg["rts_tx"], rts_level_for_rx=not cfg["rts_tx"],
                    loopback=False, delay_before_tx=0, delay_before_rx=0)
            else:
                self.serial.rts = not cfg["rts_tx"]
        except Exception:
            self.serial.close()
            raise

    def send(self, wire):
        try:
            if not self.kernel:
                self.serial.rts = self.cfg["rts_tx"]
            if self.serial.write(wire) != len(wire):
                raise IOError("舵机串口写入不完整")
            # pyserial 在 Linux 使用 tcdrain；不能只看发送队列是否为空。
            self.serial.flush()
        finally:
            if not self.kernel:
                self.serial.rts = not self.cfg["rts_tx"]

    def position(self, target):
        # 逐个地址发送，兼容不支持控制器大括号批处理的直连舵机固件。
        for joint, angle in zip(self.cfg["joints"], target.joint_rad):
            self.send(f"#{int(joint['id']):03d}P{arm_pwm(joint, angle):04d}"
                      f"T{target.duration_ms:04d}!".encode("ascii"))

    def stop(self):
        failures = []
        for joint in self.cfg["joints"]:
            try:
                self.send(f"#{int(joint['id']):03d}PDST!".encode("ascii"))
            except Exception as e:
                failures.append(str(e))
        if failures:
            raise IOError("；".join(failures))

    def read_position(self, index):
        joint = self.cfg["joints"][index]
        sid = int(joint["id"])
        wire = f"#{sid:03d}PRAD!".encode("ascii")
        self.serial.reset_input_buffer()
        self.send(wire)
        deadline = time.monotonic() + self.cfg["timeout_ms"] / 1000
        buf = bytearray()
        while time.monotonic() < deadline:
            data = self.serial.read(1)
            if data == b"#":
                buf.clear()
            if data:
                buf.extend(data)
            if len(buf) > 256:
                buf.clear()
            if data in (b"!", b"\r", b"\n"):
                reply = bytes(buf).strip(b"!\r\n")
                buf.clear()
                if reply == wire[:-1]:
                    continue
                if reply.startswith(f"#{sid:03d}PERR:".encode()):
                    raise IOError(f"舵机 {sid}：{reply.decode('ascii')}")
                m = re.fullmatch(rb"#(\d{3})P(\d{3,4})", reply)
                if m and int(m[1]) == sid and 500 <= int(m[2]) <= 2500:
                    pwm = int(m[2])
                    return dict(id=sid, pwm=pwm, position_rad=(pwm - joint["center_pwm"])
                                / (joint["direction"] * joint["pwm_per_rad"]))
        raise TimeoutError(f"舵机 {sid} 位置反馈超时")

    def close(self):
        try:
            if not self.kernel:
                self.serial.rts = not self.cfg["rts_tx"]
        finally:
            self.serial.close()


class Hardware:
    def __init__(self, cfg):
        self.cfg = cfg
        self.board = None
        self.arms = {}
        self.eye_channel = None
        self.eye = None
        self.state = {"chassis": {}, "neck": {}, "left_arm": {}, "right_arm": {}, "eyes": {}}
        self.eye_config = ParseDict(cfg["eyes"]["settings"], ep.Config())
        self.last_eye_pose = None
        self.last_eye_revision = 0
        self.chassis_times = {}
        self.requested_mode = 3
        self.poll_index = 0
        self.arm_query = None
        self.next_neck = self.next_arm = self.next_eye = 0

    def connect(self):
        try:
            if self.cfg["neck"]["enabled"] or self.cfg["chassis"]["enabled"]:
                transport = serial.Serial(self.cfg["h7"]["port"], 115200,
                    timeout=0.005, write_timeout=self.cfg["h7"]["timeout_ms"] / 1000,
                    exclusive=True if os.name == "posix" else None)
                transport.dtr = True
                try:
                    self.board = Bridge(self.cfg["h7"]["port"], transport=transport,
                                        timeout=self.cfg["h7"]["timeout_ms"] / 1000)
                except Exception:
                    transport.close()
                    raise
                self.state["h7"] = self.board.info()
                if self.cfg["neck"]["enabled"]:
                    if tuple(map(int, self.state["h7"]["firmware"].split("."))) < (1, 1, 0):
                        raise IOError("H7 固件需要 v1.1.0 或更新版本")
                    self.board.motor_stop()
                    self.board.configure(1, nominal=1000000)
                    for motor, name in ((1, "yaw"), (2, "pitch")):
                        self.board.motor_tune(motor, **self.cfg["neck"][name]["tuning"])
                if self.cfg["chassis"]["enabled"]:
                    self.board.configure(int(self.cfg["chassis"]["channel"]), nominal=500000)
                    self.board.send(int(self.cfg["chassis"]["channel"]), 0x421, b"\x01")
                    self.board.send(int(self.cfg["chassis"]["channel"]), 0x111, bytes(8))
                    self.board.send(int(self.cfg["chassis"]["channel"]), 0x141, b"\x03")
            for name in ("left_arm", "right_arm"):
                if self.cfg[name]["enabled"]:
                    self.arms[name] = ServoBus(self.cfg[name])
                    self.arms[name].stop()
            if self.cfg["eyes"]["enabled"]:
                self.eye_channel = grpc.insecure_channel(self.cfg["eyes"]["target"],
                    options=[("grpc.enable_http_proxy", 0)])
                self.eye = eg.EyeControlStub(self.eye_channel)
                self.last_eye_revision = self.eye.Configure(self.eye_config, timeout=0.3).revision
            self.poll(time.monotonic())
        except Exception:
            self.close()
            raise

    def begin_control(self):
        if self.cfg["chassis"]["enabled"]:
            self.board.send(int(self.cfg["chassis"]["channel"]), 0x421, b"\x01")

    def chassis(self, target, now):
        ch = int(self.cfg["chassis"]["channel"])
        if target.mode != self.requested_mode:
            self.board.send(ch, 0x111, bytes(8))
            self.board.send(ch, 0x141, bytes([target.mode]))
            self.requested_mode = target.mode
            # 旧模式回包不能确认刚发送的切换已完成。
            self.chassis_times.pop("mode", None)
        system = self.state["chassis"].get("system", {})
        mode = self.state["chassis"].get("mode", {})
        fresh = all(now - self.chassis_times.get(k, -1e9)
                    <= self.cfg["chassis"]["feedback_timeout_ms"] / 1000 for k in ("system", "mode"))
        ready = (fresh and system.get("state") == 0 and system.get("fault_bits") == 0
                 and system.get("control_mode") == 1 and mode.get("mode") == target.mode
                 and not mode.get("switching", True))
        self.board.send(ch, 0x111, chassis_payload(target) if ready else bytes(8))
        self.state["chassis"]["ready"] = ready

    def apply(self, targets):
        if targets.HasField("neck"):
            n = targets.neck
            converted = {}
            for name, field in (("yaw", "yaw_rad"), ("pitch", "pitch_rad")):
                if n.HasField(field):
                    c = self.cfg["neck"][name]
                    converted["gm" if name == "yaw" else "dm"] = (
                        c["direction"] * getattr(n, field) + c["offset_rad"])
            self.board.motor_position(**converted)
        for name in ("left_arm", "right_arm"):
            if targets.HasField(name):
                self.arms[name].position(getattr(targets, name))
        if targets.HasField("eyes"):
            self.last_eye_revision = self.eye.SetPose(targets.eyes, timeout=0.3).revision
            self.last_eye_pose = copy.deepcopy(targets.eyes)
            self.eye_config.mode = ep.EXTERNAL
        if targets.HasField("blink"):
            self.eye.TriggerBlink(targets.blink, timeout=0.3)

    def stop(self):
        failures = []
        operations = []
        if self.board:
            if self.cfg["chassis"]["enabled"]:
                ch = int(self.cfg["chassis"]["channel"])
                operations.extend([lambda: self.board.send(ch, 0x111, bytes(8)),
                                   lambda: self.board.send(ch, 0x141, b"\x03")])
            if self.cfg["neck"]["enabled"]:
                operations.append(self.board.motor_stop)
        operations.extend(arm.stop for arm in self.arms.values())
        for operation in operations:
            try:
                operation()
            except Exception as e:
                failures.append(str(e))
        self.requested_mode = 3
        self.chassis_times.pop("mode", None)
        if failures:
            raise IOError("停止设备失败：" + "；".join(failures))

    def stop_chassis(self):
        ch = int(self.cfg["chassis"]["channel"])
        self.board.send(ch, 0x111, bytes(8))
        self.board.send(ch, 0x141, b"\x03")
        self.requested_mode = 3
        self.chassis_times.pop("mode", None)

    def zero_neck(self, mask):
        self.board.motor_stop(mask)
        self.board.motor_zero(mask)

    def poll(self, now):
        if self.board and self.cfg["chassis"]["enabled"]:
            # 非阻塞抽取 CDC 接收缓存；限制工作量，避免遥测挤占指令周期。
            if self.board.serial.in_waiting:
                self.board._poll()
            for _ in range(256):
                frame = self.board.recv(timeout=0)
                if frame is None:
                    break
                if frame.channel != self.cfg["chassis"]["channel"] or frame.flags:
                    continue
                decoded = decode_chassis(frame.can_id, frame.data)
                if decoded:
                    key, value = decoded
                    self.state["chassis"][key] = value
                    self.chassis_times[key] = now
            self.state["chassis"]["feedback_age_ms"] = {
                k: max(0, int((now - t) * 1000)) for k, t in self.chassis_times.items()}
            self.state["chassis"]["host_rx_dropped"] = self.board.host_rx_dropped
        if self.board and self.cfg["neck"]["enabled"] and now >= self.next_neck:
            self.state["neck"] = self.board.motor_status()
            for key, name in (("gm6020", "yaw"), ("dm4310", "pitch")):
                raw = self.state["neck"][key]
                c = self.cfg["neck"][name]
                raw["joint_position_rad"] = (raw["position_rad"] - c["offset_rad"]) / c["direction"]
            self.next_neck = now + 0.2
        if self.arm_query is not None:
            name, i, wire, deadline, buf = self.arm_query
            bus = self.arms[name]
            data = bus.serial.read(min(bus.serial.in_waiting, 256))
            done = False
            error = ""
            for byte in data:
                if byte == ord("#"):
                    buf.clear()
                buf.append(byte)
                if len(buf) > 256:
                    buf.clear()
                if byte in (33, 13, 10):
                    reply = bytes(buf).strip(b"!\r\n")
                    buf.clear()
                    joint = self.cfg[name]["joints"][i]
                    sid = int(joint["id"])
                    m = re.fullmatch(rb"#(\d{3})P(\d{3,4})", reply)
                    if m and int(m[1]) == sid and 500 <= int(m[2]) <= 2500:
                        pwm = int(m[2])
                        self.state[name][str(i)] = dict(id=sid, pwm=pwm, error="",
                            position_rad=(pwm - joint["center_pwm"]) /
                                (joint["direction"] * joint["pwm_per_rad"]),
                            feedback_time=time.time())
                        done = True
                        break
                    if reply.startswith(f"#{sid:03d}PERR:".encode()):
                        error = reply.decode("ascii", errors="replace")
                        done = True
                        break
            if not done and now >= deadline:
                error, done = "位置反馈超时", True
            if done:
                if error:
                    self.state[name][str(i)] = dict(error=error, feedback_time=time.time())
                self.arm_query = None
        if self.arms and self.arm_query is None and now >= self.next_arm:
            items = [(name, i) for name in self.arms for i in range(4)]
            name, i = items[self.poll_index % len(items)]
            bus = self.arms[name]
            sid = int(self.cfg[name]["joints"][i]["id"])
            wire = f"#{sid:03d}PRAD!".encode("ascii")
            bus.serial.reset_input_buffer()
            bus.send(wire)
            self.arm_query = (name, i, wire, now + bus.cfg["timeout_ms"] / 1000, bytearray())
            self.poll_index += 1
            self.next_arm = now + 0.1
        if self.eye and now >= self.next_eye:
            state = self.eye.GetState(ep.Empty(), timeout=0.3)
            # 眼屏 UI 或自身重启造成的配置漂移由权威副本覆盖。
            if state.config != self.eye_config or state.revision < self.last_eye_revision:
                self.eye.Configure(self.eye_config, timeout=0.3)
                if self.last_eye_pose is not None:
                    self.last_eye_revision = self.eye.SetPose(self.last_eye_pose, timeout=0.3).revision
            self.state["eyes"] = MessageToDict(state, preserving_proto_field_name=True)
            self.next_eye = now + 0.5

    def close(self):
        if self.board or self.arms:
            try:
                self.stop()
            except Exception:
                pass
        for arm in self.arms.values():
            try:
                arm.close()
            except Exception:
                pass
        self.arms.clear()
        if self.board:
            self.board.close()
            self.board = None
        if self.eye_channel:
            self.eye_channel.close()
            self.eye_channel = None


class Simulated:
    """保留指令轨迹用于开发；模拟状态不冒充硬件反馈。"""
    def __init__(self, cfg):
        self.cfg = cfg
        self.state = {"events": [], "feedback_kind": "simulated"}

    def record(self, operation, value=None):
        self.state["events"].append(dict(operation=operation, value=value))
        self.state["events"] = self.state["events"][-32:]

    def connect(self):
        self.record("connect")

    def begin_control(self):
        self.record("begin_control")

    def chassis(self, target, now):
        self.record("chassis", dict(id=0x111, data=chassis_payload(target).hex(), mode=target.mode))

    def apply(self, targets):
        self.record("targets", MessageToDict(targets, preserving_proto_field_name=True))

    def stop(self):
        self.record("stop")

    def stop_chassis(self):
        self.record("stop_chassis")

    def zero_neck(self, mask):
        self.record("zero_neck", mask)

    def poll(self, now):
        pass

    def close(self):
        self.record("close")

    def snapshot(self):
        return copy.deepcopy(self.state)
