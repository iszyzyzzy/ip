"""测试启动真实 Rust 服务端，客户端仍使用公开 protobuf。"""
import os
import socket
import subprocess
import tempfile
from contextlib import ExitStack
from pathlib import Path

import grpc

from robotd.pb import robot_pb2 as pb
from robotd.pb import robot_pb2_grpc as pg

ROOT = Path(__file__).resolve().parents[2]


class RustDaemon:
    def __init__(self, token="", port=None, seed=None):
        suffix = ".exe" if os.name == "nt" else ""
        self.binary = Path(os.environ.get("ROBOTD_TEST_BIN", ROOT / f"robotd-rs/target/debug/robotd{suffix}"))
        if not self.binary.exists():
            raise RuntimeError("请先 cargo build --manifest-path robotd-rs/Cargo.toml")
        base = ROOT / ".reference"
        base.mkdir(exist_ok=True)
        self.tmp = tempfile.TemporaryDirectory(prefix="robotd-test-", dir=base)
        if port is None:
            with socket.socket() as s:
                s.bind(("127.0.0.1", 0))
                port = s.getsockname()[1]
        self.port = port
        self.token = token
        self.process = None
        self.resources = ExitStack()
        # 日志与子进程同寿命，由 close 中的 ExitStack 统一关闭。
        self.log = self.resources.enter_context(open(Path(self.tmp.name) / "server.log", "w", encoding="utf-8"))  # noqa: SIM115
        if seed:
            seed(Path(self.tmp.name) / "state.sqlite")
        self.start()

    def start(self):
        env = dict(os.environ, ROBOTD_API_TOKEN=self.token)
        self.process = subprocess.Popen([
            str(self.binary), "--simulate", "--listen", f"127.0.0.1:{self.port}",
            "--db", str(Path(self.tmp.name) / "state.sqlite"),
        ], stdout=self.log, stderr=self.log, env=env,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        self.channel = grpc.insecure_channel(f"127.0.0.1:{self.port}",
                                             options=[("grpc.enable_http_proxy", 0)])
        try:
            grpc.channel_ready_future(self.channel).result(timeout=5)
        except grpc.FutureTimeoutError:
            self.stop()
            self.log.flush()
            raise RuntimeError(Path(self.log.name).read_text(encoding="utf-8")) from None
        self.stub = pg.RobotControlStub(self.channel)

    def stop(self):
        self.channel.close()
        if self.process is not None and self.process.poll() is None:
            self.process.terminate()
            self.process.wait(timeout=5)
        self.process = None

    def state(self):
        metadata = (("authorization", f"Bearer {self.token}"),) if self.token else ()
        return self.stub.GetState(pb.Empty(), timeout=3, metadata=metadata)

    def close(self):
        self.stop()
        self.resources.close()
        self.tmp.cleanup()
