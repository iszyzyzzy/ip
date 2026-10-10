"""robotd gRPC 入口与进程生命周期。"""
from __future__ import annotations
import argparse
from concurrent import futures
import json
import logging
import os
from pathlib import Path
import signal
import threading
import grpc
from google.protobuf.json_format import ParseError
from .control import Controller, ControlError
from .pb import robot_pb2 as pb, robot_pb2_grpc as pg
from .store import Store, Conflict


class Service(pg.RobotControlServicer):
    def __init__(self, controller, api_token=""):
        self.controller = controller
        self.api_token = api_token
        self.stream_slots = threading.BoundedSemaphore(8)

    def authorize(self, context):
        import secrets
        token = dict(context.invocation_metadata()).get("authorization", "")
        if self.api_token and not secrets.compare_digest(token, f"Bearer {self.api_token}"):
            context.abort(grpc.StatusCode.UNAUTHENTICATED, "访问凭据无效")

    def call(self, context, method, *args, urgent=False):
        self.authorize(context)
        if not context.is_active():
            context.abort(grpc.StatusCode.CANCELLED, "请求已取消")
        remaining = context.time_remaining()
        timeout = min(5, remaining) if remaining is not None else 5
        try:
            return self.controller.call(method, *args, timeout=max(0.001, timeout), urgent=urgent)
        except ControlError as e:
            context.abort(e.code, str(e))
        except Conflict as e:
            context.abort(grpc.StatusCode.ABORTED, str(e))
        except (ValueError, ParseError, TypeError) as e:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, str(e))
        except KeyError as e:
            context.abort(grpc.StatusCode.NOT_FOUND, f"动作不存在：{e}")
        except Exception as e:
            logging.exception("RPC 执行失败：%s", method)
            context.abort(grpc.StatusCode.INTERNAL, str(e))

    def GetConfig(self, request, context):
        return self.call(context, "get_config")

    def UpdateConfig(self, request, context):
        return self.call(context, "update_config", request)

    def GetState(self, request, context):
        self.authorize(context)
        return self.controller.state()

    def WatchState(self, request, context):
        self.authorize(context)
        interval = request.interval_ms or 100
        if not 50 <= interval <= 5000:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "interval_ms 必须在 50..5000")
        cancelled = threading.Event()
        context.add_callback(cancelled.set)
        if not self.stream_slots.acquire(blocking=False):
            context.abort(grpc.StatusCode.RESOURCE_EXHAUSTED, "状态/控制流数量已满")
        try:
            while context.is_active() and not self.controller.done.is_set():
                yield self.controller.state()
                if cancelled.wait(interval / 1000):
                    break
        finally:
            self.stream_slots.release()

    def AcquireControl(self, request, context):
        return self.call(context, "acquire", request)

    def RenewControl(self, request, context):
        return self.call(context, "renew", request)

    def ReleaseControl(self, request, context):
        return self.call(context, "release", request, urgent=True)

    def SetTargets(self, request, context):
        return self.call(context, "set_targets", request)

    def StreamTargets(self, requests, context):
        self.authorize(context)
        last = pb.Ack()
        if not self.stream_slots.acquire(blocking=False):
            context.abort(grpc.StatusCode.RESOURCE_EXHAUSTED, "状态/控制流数量已满")
        try:
            for request in requests:
                last = self.call(context, "set_targets", request)
            return last
        finally:
            self.stream_slots.release()

    def Stop(self, request, context):
        return self.call(context, "stop", request, urgent=True)

    def ZeroNeck(self, request, context):
        return self.call(context, "zero_neck", request)

    def ListActions(self, request, context):
        return self.call(context, "list_actions")

    def PutAction(self, request, context):
        return self.call(context, "put_action", request)

    def DeleteAction(self, request, context):
        return self.call(context, "delete_action", request)

    def RunAction(self, request, context):
        return self.call(context, "run_action", request)

    def CancelAction(self, request, context):
        return self.call(context, "cancel_action", request, urgent=True)


class InstanceLock:
    def __init__(self, path):
        self.file = open(str(path) + ".lock", "a+b")
        try:
            self.file.seek(0)
            if self.file.read(1) == b"":
                self.file.write(b"0")
                self.file.flush()
            self.file.seek(0)
            if os.name == "nt":
                import msvcrt
                msvcrt.locking(self.file.fileno(), msvcrt.LK_NBLCK, 1)
            else:
                import fcntl
                fcntl.flock(self.file.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            self.file.close()
            raise RuntimeError("此数据库已由另一个 robotd 使用") from None

    def close(self):
        self.file.close()


def serve(controller, listen, token=""):
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=16),
                         maximum_concurrent_rpcs=32,
                         options=[("grpc.max_receive_message_length", 1024 * 1024),
                                  ("grpc.max_send_message_length", 1024 * 1024)])
    pg.add_RobotControlServicer_to_server(Service(controller, token), server)
    if server.add_insecure_port(listen) == 0:
        raise RuntimeError(f"无法监听 {listen}")
    server.start()
    return server


def main():
    p = argparse.ArgumentParser(description="机器人统一控制与配置服务")
    p.add_argument("--db", default="robotd.sqlite", help="唯一配置数据库")
    p.add_argument("--init", type=Path, help="仅新数据库：从 JSON 导入初始配置")
    p.add_argument("--listen", default="127.0.0.1:50051")
    p.add_argument("--simulate", action="store_true", help="模拟设备，用于上层开发")
    args = p.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    path = Path(args.db).resolve()
    path.parent.mkdir(parents=True, exist_ok=True)
    lock = InstanceLock(path)
    controller = server = None
    try:
        initial = json.loads(args.init.read_text(encoding="utf-8")) if args.init else None
        controller = Controller(Store(path, initial), simulated=args.simulate)
        controller.start()
        server = serve(controller, args.listen, os.environ.get("ROBOTD_API_TOKEN", ""))
        logging.info("robotd 已启动：%s，配置数据库：%s，模拟：%s",
                     args.listen, path, args.simulate)
        stopped = threading.Event()
        for sig in (signal.SIGINT, signal.SIGTERM):
            signal.signal(sig, lambda *_: stopped.set())
        while not stopped.wait(0.5):
            if controller.done.is_set():
                raise RuntimeError("控制线程已退出")
    finally:
        if server:
            server.stop(0).wait(2)
        if controller:
            controller.close()
        lock.close()


if __name__ == "__main__":
    main()
