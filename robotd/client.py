"""robotctl：配置、调试和动作开发命令行。"""
from __future__ import annotations

import argparse
import json
import os
import threading
import time
from pathlib import Path

import grpc
from google.protobuf.json_format import MessageToDict, ParseDict

from .config import default_config
from .pb import robot_pb2 as pb
from .pb import robot_pb2_grpc as pg


def show(message):
    print(json.dumps(MessageToDict(message, preserving_proto_field_name=True,
                                  always_print_fields_with_no_presence=True),
                     ensure_ascii=False, indent=2))


class Session:
    """获得控制权并续期；退出时停止设备。上层程序可直接复用。"""
    def __init__(self, stub, owner="robotctl", metadata=(), clear_latch=False):
        self.stub = stub
        self.metadata = metadata
        self.lease = stub.AcquireControl(pb.AcquireRequest(owner=owner, ttl_ms=1000,
                            clear_latch=clear_latch), timeout=5, metadata=metadata)
        self.done = threading.Event()
        self.error = None
        self.thread = threading.Thread(target=self._renew, daemon=True)
        self.thread.start()

    def _renew(self):
        while not self.done.wait(self.lease.ttl_ms / 3000):
            try:
                self.stub.RenewControl(pb.LeaseRequest(token=self.lease.token),
                                      timeout=0.8, metadata=self.metadata)
            except grpc.RpcError as e:
                self.error = e
                self.done.set()

    def check(self):
        if self.error:
            raise self.error

    def send(self, targets):
        self.check()
        return self.stub.SetTargets(pb.TargetRequest(token=self.lease.token, targets=targets),
                                   timeout=5, metadata=self.metadata)

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.done.set()
        self.thread.join(timeout=2)
        try:
            self.stub.ReleaseControl(pb.LeaseRequest(token=self.lease.token),
                                     timeout=5, metadata=self.metadata)
        except grpc.RpcError:
            pass


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--target", default="127.0.0.1:50051")
    p.add_argument("--clear-latch", action="store_true")
    commands = p.add_subparsers(dest="command", required=True)
    commands.add_parser("state")
    commands.add_parser("watch")
    commands.add_parser("config")
    default = commands.add_parser("default-config")
    default.add_argument("file", type=Path)
    update = commands.add_parser("update-config")
    update.add_argument("file", type=Path, help="GetConfig 导出的含 revision/config 的 JSON")
    stop = commands.add_parser("stop")
    stop.add_argument("--latch", action="store_true")
    zero = commands.add_parser("zero-neck")
    zero.add_argument("--mask", type=int, choices=(1, 2, 3), default=3)
    commands.add_parser("actions")
    put = commands.add_parser("put-action")
    put.add_argument("file", type=Path)
    put.add_argument("--revision", type=int, default=0)
    delete = commands.add_parser("delete-action")
    delete.add_argument("name")
    delete.add_argument("--revision", type=int, required=True)
    run = commands.add_parser("run")
    run.add_argument("name")
    send = commands.add_parser("send")
    send.add_argument("file", type=Path, help="Targets JSON")
    send.add_argument("--seconds", type=float, default=2)
    args = p.parse_args()
    if args.command == "default-config":
        args.file.write_text(json.dumps(default_config(), ensure_ascii=False, indent=2),
                             encoding="utf-8")
        return
    metadata = (("authorization", "Bearer " + os.environ["ROBOTD_API_TOKEN"]),) \
        if os.environ.get("ROBOTD_API_TOKEN") else ()
    with grpc.insecure_channel(args.target, options=[("grpc.enable_http_proxy", 0)]) as channel:
        stub = pg.RobotControlStub(channel)
        options = {"timeout": 5, "metadata": metadata}
        if args.command == "state":
            show(stub.GetState(pb.Empty(), **options))
        elif args.command == "watch":
            for state in stub.WatchState(pb.WatchRequest(interval_ms=200), metadata=metadata):
                show(state)
        elif args.command == "config":
            show(stub.GetConfig(pb.Empty(), **options))
        elif args.command == "update-config":
            snapshot = ParseDict(json.loads(args.file.read_text(encoding="utf-8")), pb.ConfigSnapshot())
            show(stub.UpdateConfig(pb.ConfigUpdate(expected_revision=snapshot.revision,
                                                  config=snapshot.config), **options))
        elif args.command == "stop":
            show(stub.Stop(pb.StopRequest(latch=args.latch), **options))
        elif args.command == "actions":
            show(stub.ListActions(pb.Empty(), **options))
        elif args.command == "put-action":
            action = ParseDict(json.loads(args.file.read_text(encoding="utf-8")), pb.Action())
            show(stub.PutAction(pb.ActionUpdate(expected_revision=args.revision, action=action),
                                **options))
        elif args.command == "delete-action":
            show(stub.DeleteAction(pb.ActionDelete(name=args.name, expected_revision=args.revision),
                                   **options))
        else:
            with Session(stub, metadata=metadata, clear_latch=args.clear_latch) as session:
                if args.command == "zero-neck":
                    show(stub.ZeroNeck(pb.ZeroRequest(token=session.lease.token, mask=args.mask),
                                       **options))
                elif args.command == "run":
                    stub.RunAction(pb.RunRequest(token=session.lease.token, name=args.name), **options)
                    while True:
                        session.check()
                        state = stub.GetState(pb.Empty(), **options)
                        if state.fault:
                            raise RuntimeError(state.fault)
                        if not state.action:
                            show(state)
                            break
                        time.sleep(0.1)
                else:
                    if not 0 < args.seconds <= 600:
                        p.error("--seconds 必须在 0..600 之间")
                    targets = ParseDict(json.loads(args.file.read_text(encoding="utf-8")), pb.Targets())
                    end = time.monotonic() + args.seconds
                    show(session.send(targets))
                    while time.monotonic() < end:
                        time.sleep(0.02)
                        session.check()
                        if targets.HasField("chassis"):
                            session.send(pb.Targets(chassis=targets.chassis))


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
    except grpc.RpcError as e:
        raise SystemExit(f"{e.code().name}: {e.details()}") from None
