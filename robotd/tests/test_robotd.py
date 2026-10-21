"""用既有 Python gRPC 协议联调真实 Rust daemon。"""
import json
import sqlite3
import threading
import time
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer

import grpc
from google.protobuf.json_format import MessageToDict, ParseDict

from robotd.audio_client import tts_packets
from robotd.config import default_config
from robotd.pb import robot_pb2 as pb
from robotd.tests.support import ROOT, RustDaemon


class RobotTests(unittest.TestCase):
    def setUp(self):
        self.backend = RustDaemon()
        self.stub = self.backend.stub

    def tearDown(self):
        self.backend.close()

    def enable_audio(self):
        cfg = self.stub.GetConfig(pb.Empty(), timeout=3)
        cfg.config["audio"]["enabled"] = True
        return self.stub.UpdateConfig(pb.ConfigUpdate(expected_revision=cfg.revision,
                                                      config=cfg.config), timeout=3)

    def wait_audio(self, predicate):
        end = time.monotonic() + 2
        while time.monotonic() < end:
            state = self.stub.GetAudioState(pb.Empty(), timeout=1)
            if predicate(state):
                return state
            time.sleep(0.005)
        self.fail(f"音频状态未满足条件：{state}")

    def test_motion_config_actions_and_persistence(self):
        cfg = self.stub.GetConfig(pb.Empty(), timeout=3)
        self.assertEqual(cfg.config["schema_version"], 2)
        cfg.config["metadata"]["name"] = "Rust 机器人"
        self.stub.UpdateConfig(pb.ConfigUpdate(expected_revision=cfg.revision, config=cfg.config), timeout=3)
        with self.assertRaises(grpc.RpcError) as conflict:
            self.stub.UpdateConfig(pb.ConfigUpdate(expected_revision=cfg.revision, config=cfg.config), timeout=3)
        self.assertEqual(conflict.exception.code(), grpc.StatusCode.ABORTED)
        lease = self.stub.AcquireControl(pb.AcquireRequest(owner="test", ttl_ms=1000), timeout=3)
        target = pb.TargetRequest(token=lease.token, targets=pb.Targets(
            neck=pb.NeckTarget(yaw_rad=0.1), chassis=pb.ChassisTarget(speed_m_s=0.15, ttl_ms=100)))
        self.stub.SetTargets(target, timeout=3)
        time.sleep(0.13)
        self.stub.RenewControl(pb.LeaseRequest(token=lease.token), timeout=3)
        state = self.backend.state()
        events = MessageToDict(state.devices)["events"]
        self.assertIn("stop_chassis", [e["operation"] for e in events])
        self.assertEqual(state.owner, "test")
        invalid = pb.TargetRequest(token=lease.token, targets=pb.Targets(
            neck=pb.NeckTarget(yaw_rad=0.1), left_arm=pb.ArmTarget(joint_rad=[10]*4, duration_ms=100)))
        with self.assertRaises(grpc.RpcError) as bad:
            self.stub.SetTargets(invalid, timeout=3)
        self.assertEqual(bad.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)
        action = ParseDict(json.loads((ROOT / "robotd/examples/greeting.json").read_text()), pb.Action())
        self.stub.PutAction(pb.ActionUpdate(action=action), timeout=3)
        self.stub.RunAction(pb.RunRequest(token=lease.token, name=action.name), timeout=3)
        self.stub.CancelAction(pb.LeaseRequest(token=lease.token), timeout=3)
        action.duration_ms = 100
        del action.steps[1:]
        self.stub.PutAction(pb.ActionUpdate(action=action, expected_revision=1), timeout=3)
        self.stub.RunAction(pb.RunRequest(token=lease.token, name=action.name), timeout=3)
        end = time.monotonic() + 1
        while self.backend.state().action and time.monotonic() < end:
            time.sleep(0.01)
        self.assertTrue(self.backend.state().stopped)
        self.assertFalse(self.backend.state().action)
        self.stub.DeleteAction(pb.ActionDelete(name=action.name, expected_revision=2), timeout=3)
        self.assertTrue(self.stub.ListActions(pb.Empty(), timeout=3).actions[0].deleted)
        self.stub.Stop(pb.StopRequest(latch=True), timeout=3)
        self.assertTrue(self.backend.state().latched)
        self.assertFalse(self.backend.state().owner)
        self.backend.stop()
        self.backend.start()
        cfg = self.backend.stub.GetConfig(pb.Empty(), timeout=3)
        self.assertEqual(cfg.config["metadata"]["name"], "Rust 机器人")
        self.assertTrue(self.backend.stub.ListActions(pb.Empty(), timeout=3).actions[0].deleted)
        self.assertTrue(self.backend.state().stopped)

    def test_audio_bounded_buffer_arbitrary_fragments_and_tail(self):
        self.enable_audio()
        data = bytes(range(256)) * 100  # 400 ms，显著超过队列上限。
        def packets():
            yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=32000, channels=1))
            for i in range(0, len(data), 997):
                yield pb.AudioPacket(pcm=data[i:i+997])
        begin = time.monotonic()
        result = self.stub.PlayAudio(packets(), timeout=3)
        self.assertGreater(time.monotonic() - begin, 0.35)
        self.assertEqual(result.phase, "completed")
        self.assertEqual(result.received_frames, len(data)//2)
        self.assertEqual(result.submitted_frames, len(data)//2)
        self.assertLessEqual(result.peak_buffered_ms, 160.01)
        self.assertLess(result.start_latency_ms, 200)
        self.assertEqual(result.buffered_ms, 0)
        # EOS 必须立即播放短尾部，不能等待永远达不到的预缓冲阈值。
        result = self.stub.PlayAudio(iter([
            pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=32000, channels=1)),
            pb.AudioPacket(pcm=bytes(640)),
        ]), timeout=1)
        self.assertEqual(result.submitted_frames, 320)
        self.assertEqual(result.phase, "completed")

    def test_audio_rebuffer_gap_and_motor_control_independence(self):
        self.enable_audio()
        def packets():
            yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=32000, channels=1))
            yield pb.AudioPacket(pcm=bytes(6400))
            time.sleep(0.25)
            yield pb.AudioPacket(pcm=bytes(12800))
        future = self.stub.PlayAudio.future(packets(), timeout=3)
        self.wait_audio(lambda s: s.phase in ("playing", "rebuffering"))
        lease = self.stub.AcquireControl(pb.AcquireRequest(owner="remote", ttl_ms=200), timeout=1)
        self.stub.SetTargets(pb.TargetRequest(token=lease.token,
            targets=pb.Targets(neck=pb.NeckTarget(yaw_rad=0.1))), timeout=1)
        # 运动租约到期不会截断独立的语音流。
        result = future.result(timeout=3)
        self.assertGreaterEqual(result.rebuffer_count, 1)
        self.assertGreater(result.silence_frames, 0)
        self.assertEqual(result.submitted_frames, 9600)
        self.assertFalse(self.backend.state().owner)

    def test_audio_stop_cancel_and_single_stream(self):
        self.enable_audio()
        release = threading.Event()
        def packets():
            yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=32000, channels=1))
            yield pb.AudioPacket(pcm=bytes(9600))
            release.wait(2)
            yield pb.AudioPacket(pcm=bytes(640))
        future = self.stub.PlayAudio.future(packets(), timeout=3)
        try:
            self.wait_audio(lambda s: s.phase in ("playing", "rebuffering"))
            with self.assertRaises(grpc.RpcError) as busy:
                self.stub.PlayAudio(iter([pb.AudioPacket(header=pb.AudioHeader(
                    sample_rate_hz=32000, channels=1))]), timeout=1)
            self.assertEqual(busy.exception.code(), grpc.StatusCode.RESOURCE_EXHAUSTED)
            start = time.monotonic()
            self.stub.StopAudio(pb.Empty(), timeout=1)
            self.wait_audio(lambda s: s.phase == "cancelled")
            self.assertLess(time.monotonic() - start, 0.2)
            with self.assertRaises(grpc.RpcError) as stopped:
                future.result(timeout=1)
            self.assertEqual(stopped.exception.code(), grpc.StatusCode.CANCELLED)
            release.set()
            future = self.stub.PlayAudio.future(packets(), timeout=3)
            self.wait_audio(lambda s: s.phase in ("playing", "rebuffering"))
            future.cancel()
            self.wait_audio(lambda s: s.phase == "cancelled")
        finally:
            release.set()
            future.cancel()

    def test_audio_validation_and_global_stop(self):
        with self.assertRaises(grpc.RpcError) as disabled:
            self.stub.PlayAudio(iter([pb.AudioPacket(header=pb.AudioHeader(
                sample_rate_hz=32000, channels=1))]), timeout=1)
        self.assertEqual(disabled.exception.code(), grpc.StatusCode.FAILED_PRECONDITION)
        self.enable_audio()
        for messages in ([pb.AudioPacket(pcm=b"\0\0")],
                         [pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=32000, channels=1)),
                          pb.AudioPacket(pcm=b"\0")]):
            with self.assertRaises(grpc.RpcError) as bad:
                self.stub.PlayAudio(iter(messages), timeout=1)
            self.assertEqual(bad.exception.code(), grpc.StatusCode.INVALID_ARGUMENT)
        self.wait_audio(lambda s: s.phase in ("idle", "cancelled"))
        self.stub.Stop(pb.StopRequest(latch=True), timeout=1)
        with self.assertRaises(grpc.RpcError) as locked:
            self.stub.PlayAudio(iter([pb.AudioPacket(header=pb.AudioHeader(
                sample_rate_hz=32000, channels=1))]), timeout=1)
        self.assertEqual(locked.exception.code(), grpc.StatusCode.FAILED_PRECONDITION)

    def test_gpt_sovits_raw_http_forwarding(self):
        self.enable_audio()
        requests = []
        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                requests.append(json.loads(self.rfile.read(int(self.headers["Content-Length"]))))
                self.send_response(200)
                self.send_header("Content-Type", "audio/raw")
                self.end_headers()
                for _ in range(4):
                    self.wfile.write(bytes(1600))
                    self.wfile.flush()
                    time.sleep(0.015)

            def log_message(self, *args):
                pass

        server = HTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            packets = tts_packets(f"http://127.0.0.1:{server.server_port}/tts", {"text": "你好"}, 32000)
            result = self.stub.PlayAudio(packets, timeout=2)
            self.assertEqual(result.submitted_frames, 3200)
            self.assertEqual(requests[0]["media_type"], "raw")
            self.assertTrue(requests[0]["streaming_mode"])
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=2)


class MigrationTests(unittest.TestCase):
    def test_existing_database_and_auth(self):
        def seed(path):
            cfg = default_config()
            cfg.pop("audio")
            cfg["schema_version"] = 1
            with sqlite3.connect(path) as db:
                db.execute("CREATE TABLE config(id INTEGER PRIMARY KEY,revision INTEGER,body TEXT)")
                db.execute("INSERT INTO config VALUES(1,7,?)", (json.dumps(cfg),))
        backend = RustDaemon(token="test-secret", seed=seed)
        try:
            with self.assertRaises(grpc.RpcError) as denied:
                backend.stub.GetConfig(pb.Empty(), timeout=1)
            self.assertEqual(denied.exception.code(), grpc.StatusCode.UNAUTHENTICATED)
            cfg = backend.stub.GetConfig(pb.Empty(), timeout=1,
                metadata=(("authorization", "Bearer test-secret"),))
            self.assertEqual(cfg.revision, 8)
            self.assertEqual(cfg.config["schema_version"], 2)
            self.assertFalse(cfg.config["audio"]["enabled"])
        finally:
            backend.close()


if __name__ == "__main__":
    unittest.main()
