"""独立运行的网页遥控服务；设备、配置和运动时序仍由 robotd 管理。"""
from __future__ import annotations

import argparse
import asyncio
from contextlib import suppress
from dataclasses import dataclass, field
import json
import logging
import os
from pathlib import Path
import secrets
import time
from urllib.parse import urlsplit

from aiohttp import WSMsgType, web
import grpc
from google.protobuf.json_format import MessageToDict, ParseDict, ParseError

from robotd.pb import robot_pb2 as pb, robot_pb2_grpc as pg

log = logging.getLogger(__name__)
MAX_MESSAGE_SIZE = 1024 * 1024
HTTP_STATUS = {
    "INVALID_ARGUMENT": 400, "UNAUTHENTICATED": 401, "PERMISSION_DENIED": 403,
    "NOT_FOUND": 404, "ABORTED": 409, "FAILED_PRECONDITION": 409,
    "RESOURCE_EXHAUSTED": 429, "UNAVAILABLE": 503, "DEADLINE_EXCEEDED": 504,
}
# HTTP 与 WebSocket 共用命令表，避免两种入口的控制语义发生漂移。
COMMANDS = {
    "get_state": ("GetState", pb.Empty, False),
    "get_config": ("GetConfig", pb.Empty, False),
    "update_config": ("UpdateConfig", pb.ConfigUpdate, False),
    "list_actions": ("ListActions", pb.Empty, False),
    "put_action": ("PutAction", pb.ActionUpdate, False),
    "delete_action": ("DeleteAction", pb.ActionDelete, False),
    "acquire": ("AcquireControl", pb.AcquireRequest, False),
    "renew": ("RenewControl", pb.LeaseRequest, True),
    "release": ("ReleaseControl", pb.LeaseRequest, True),
    "targets": ("SetTargets", pb.TargetRequest, True),
    "stop": ("Stop", pb.StopRequest, False),
    "zero_neck": ("ZeroNeck", pb.ZeroRequest, True),
    "run_action": ("RunAction", pb.RunRequest, True),
    "cancel_action": ("CancelAction", pb.LeaseRequest, True),
}
ROUTES = [
    ("GET", "/api/state", "get_state"),
    ("GET", "/api/config", "get_config"),
    ("PUT", "/api/config", "update_config"),
    ("GET", "/api/actions", "list_actions"),
    ("POST", "/api/actions", "put_action"),
    ("DELETE", "/api/actions", "delete_action"),
    ("POST", "/api/control/acquire", "acquire"),
    ("POST", "/api/control/renew", "renew"),
    ("POST", "/api/control/release", "release"),
    ("POST", "/api/targets", "targets"),
    ("POST", "/api/stop", "stop"),
    ("POST", "/api/neck/zero", "zero_neck"),
    ("POST", "/api/actions/run", "run_action"),
    ("POST", "/api/actions/cancel", "cancel_action"),
]


class RemoteError(Exception):
    def __init__(self, code, message, status=None):
        super().__init__(message)
        self.code = code
        self.status = status or HTTP_STATUS.get(code, 500)

    def payload(self):
        return {"code": self.code, "message": str(self)}


def message_dict(message):
    return MessageToDict(message, preserving_proto_field_name=True,
                         always_print_fields_with_no_presence=True)


def json_object(value):
    if not isinstance(value, dict):
        raise RemoteError("INVALID_ARGUMENT", "JSON 必须是对象")
    return value


def load_json(value):
    # JSON 的 NaN/Infinity 不是合法数值，也不能成为运动目标。
    def reject_constant(constant):
        raise ValueError(f"非法 JSON 数值：{constant}")
    try:
        return json_object(json.loads(value, parse_constant=reject_constant))
    except (ValueError, TypeError) as error:
        raise RemoteError("INVALID_ARGUMENT", f"JSON 无效：{error}") from None


def origin_key(value):
    try:
        url = urlsplit(value)
        if (url.scheme not in ("http", "https") or not url.hostname or url.username
                or url.password or url.path not in ("", "/") or url.query or url.fragment):
            raise ValueError("需使用 http(s)://主机[:端口]")
        return url.scheme, url.hostname.lower(), url.port or (443 if url.scheme == "https" else 80)
    except ValueError as error:
        raise ValueError(f"网页来源无效：{value}，{error}") from None


@dataclass
class Session:
    token: str
    owner: str
    ttl_ms: int
    deadline: float
    peer: object = None

    def public(self, session_id):
        return {"session_id": session_id, "owner": self.owner, "ttl_ms": self.ttl_ms}


@dataclass(eq=False)
class Peer:
    ws: web.WebSocketResponse
    queue: asyncio.Queue = field(default_factory=lambda: asyncio.Queue(maxsize=1))
    send_lock: asyncio.Lock = field(default_factory=asyncio.Lock)
    session_id: str = ""

    async def send(self, value):
        async with self.send_lock:
            await asyncio.wait_for(self.ws.send_json(value), timeout=2)


class Gateway:
    def __init__(self, target, robotd_token="", api_token="", allowed_origins=(),
                 state_interval_ms=100, rpc_timeout=2, max_clients=16):
        self.target = target
        self.metadata = (("authorization", f"Bearer {robotd_token}"),) if robotd_token else ()
        self.api_token = api_token
        self.allowed_origins = {origin_key(origin) for origin in allowed_origins}
        self.state_interval_ms = state_interval_ms
        self.rpc_timeout = rpc_timeout
        self.max_clients = max_clients
        self.sessions = {}
        self.peers = set()
        self.subscribers = set()
        self.channel = self.stub = None
        self.latest = None
        self.closing = False

    def authorized(self, authorization):
        return not self.api_token or secrets.compare_digest(
            authorization.encode("utf-8"), f"Bearer {self.api_token}".encode("utf-8"))

    def check_origin(self, request):
        origin = request.headers.get("Origin")
        if origin is None:
            return
        try:
            key = origin_key(origin)
            same = origin_key(f"{request.scheme}://{request.host}")
        except ValueError:
            raise RemoteError("PERMISSION_DENIED", "网页来源无效") from None
        if key != same and key not in self.allowed_origins:
            raise RemoteError("PERMISSION_DENIED", "网页来源未获允许")

    async def rpc(self, method, request):
        try:
            return await getattr(self.stub, method)(
                request, timeout=self.rpc_timeout, metadata=self.metadata)
        except grpc.aio.AioRpcError as error:
            raise RemoteError(error.code().name, error.details()) from None

    def expire_sessions(self):
        now = time.monotonic()
        for session_id, session in list(self.sessions.items()):
            if now >= session.deadline:
                self.forget(session_id)

    def forget(self, session_id):
        session = self.sessions.pop(session_id, None)
        if session and session.peer and session.peer.session_id == session_id:
            session.peer.session_id = ""

    def session(self, session_id, peer):
        if not isinstance(session_id, str) or not session_id:
            raise RemoteError("INVALID_ARGUMENT", "缺少 session_id，请先 acquire")
        session = self.sessions.get(session_id)
        if not session:
            raise RemoteError("PERMISSION_DENIED", "控制会话不存在或已到期")
        if session.peer is not peer:
            raise RemoteError("PERMISSION_DENIED", "控制会话属于另一条连接")
        return session

    async def execute(self, method, params, peer=None):
        if self.closing:
            raise RemoteError("UNAVAILABLE", "遥控服务正在退出")
        if not isinstance(method, str) or method not in COMMANDS:
            raise RemoteError("NOT_FOUND", "命令不存在")
        self.expire_sessions()
        data = dict(json_object(params))
        rpc_method, request_type, needs_session = COMMANDS[method]
        session_id = None
        session = None
        if needs_session:
            session_id = data.pop("session_id", peer.session_id if peer else "")
            session = self.session(session_id, peer)
            if "token" in data:
                raise RemoteError("INVALID_ARGUMENT", "使用 session_id，不接受 robotd token")
            data["token"] = session.token
        if method == "acquire":
            if peer and peer.session_id:
                raise RemoteError("FAILED_PRECONDITION", "此连接已经持有控制权")
            data.setdefault("owner", "web-remote")
            data.setdefault("ttl_ms", 1000)
        try:
            request = ParseDict(data, request_type())
        except (ParseError, ValueError, TypeError) as error:
            raise RemoteError("INVALID_ARGUMENT", f"参数无效：{error}") from None
        started = time.monotonic()
        revoked_ids = tuple(self.sessions) if method in ("stop", "update_config") else ()
        try:
            result = await self.rpc(rpc_method, request)
        except RemoteError as error:
            if session_id and error.code == "PERMISSION_DENIED":
                self.forget(session_id)
            raise
        if method == "acquire":
            session_id = secrets.token_urlsafe(24)
            session = Session(result.token, result.owner, result.ttl_ms,
                              started + result.ttl_ms / 1000, peer)
            self.sessions[session_id] = session
            if peer:
                peer.session_id = session_id
            return session.public(session_id)
        if method == "renew":
            session.deadline = started + result.ttl_ms / 1000
            return session.public(session_id)
        if method == "release":
            self.forget(session_id)
        # 仅删除调用开始时已有的会话；并发取得的新控制权不受本地清理影响。
        for old_id in revoked_ids:
            self.forget(old_id)
        return message_dict(result)

    def publish(self, value):
        self.latest = value
        for peer in tuple(self.subscribers):
            # 慢网页只保留最新状态，不能拖住其他网页或占用无限内存。
            if peer.queue.full():
                peer.queue.get_nowait()
            peer.queue.put_nowait(value)

    async def watch(self):
        failed = False
        while True:
            try:
                # 共用一个有超时的状态查询，网页数量不会耗尽 robotd 的流名额。
                state = await self.rpc("GetState", pb.Empty())
                self.expire_sessions()
                self.publish({"type": "state", "state": message_dict(state)})
                if failed:
                    log.info("已重新连接 robotd：%s", self.target)
                failed = False
            except RemoteError as error:
                self.publish({"type": "backend_error", "error": error.payload()})
                if not failed:
                    log.warning("robotd 连接失败：%s", error)
                failed = True
            await asyncio.sleep(1 if failed else self.state_interval_ms / 1000)

    async def send_states(self, peer):
        try:
            while True:
                await peer.send(await peer.queue.get())
        except (ConnectionError, RuntimeError, asyncio.TimeoutError):
            await peer.ws.close(code=1013, message="网页接收状态超时".encode())

    async def release_peer(self, peer):
        session_id = peer.session_id
        session = self.sessions.get(session_id)
        self.forget(session_id)
        if session:
            try:
                await self.rpc("ReleaseControl", pb.LeaseRequest(token=session.token))
            except RemoteError as error:
                if error.code not in ("PERMISSION_DENIED", "FAILED_PRECONDITION"):
                    log.warning("断开连接后释放控制权失败：%s", error)

    async def websocket(self, request):
        if self.closing:
            raise RemoteError("UNAVAILABLE", "遥控服务正在退出")
        if len(self.peers) >= self.max_clients:
            raise RemoteError("RESOURCE_EXHAUSTED", "网页连接数量已满")
        ws = web.WebSocketResponse(heartbeat=10, max_msg_size=MAX_MESSAGE_SIZE)
        peer = Peer(ws)
        self.peers.add(peer)
        sender = None
        authenticated = self.authorized(request.headers.get("Authorization", ""))
        try:
            await ws.prepare(request)
            await peer.send({"type": "ready", "protocol": "robot-remote.v1",
                             "authenticated": authenticated,
                             "state_interval_ms": self.state_interval_ms})

            def subscribe():
                self.subscribers.add(peer)
                if self.latest is not None:
                    peer.queue.put_nowait(self.latest)
                return asyncio.create_task(self.send_states(peer))

            if authenticated:
                sender = subscribe()
            while not ws.closed:
                # 未认证连接不能长时间占用名额，也不会提前收到机器人状态。
                try:
                    frame = await ws.receive(timeout=None if authenticated else 5)
                except asyncio.TimeoutError:
                    await ws.close(code=1008, message="认证超时".encode())
                    break
                if frame.type in (WSMsgType.CLOSE, WSMsgType.CLOSED, WSMsgType.CLOSING,
                                  WSMsgType.ERROR):
                    break
                request_id = None
                try:
                    if frame.type != WSMsgType.TEXT:
                        raise RemoteError("INVALID_ARGUMENT", "只接受文本 JSON 消息")
                    envelope = load_json(frame.data)
                    request_id = envelope.get("id")
                    if (isinstance(request_id, bool) or not isinstance(request_id, (str, int))
                            or isinstance(request_id, str) and len(request_id) > 128):
                        request_id = None
                        raise RemoteError("INVALID_ARGUMENT", "id 必须是整数或不超过 128 字符的字符串")
                    if set(envelope) - {"id", "method", "params"}:
                        raise RemoteError("INVALID_ARGUMENT", "消息仅接受 id、method、params")
                    method = envelope.get("method")
                    params = json_object(envelope.get("params", {}))
                    if method == "auth":
                        token = params.get("token")
                        if set(params) != {"token"} or not isinstance(token, str):
                            raise RemoteError("INVALID_ARGUMENT", "auth 需要字符串 token")
                        if not self.authorized(f"Bearer {token}"):
                            raise RemoteError("UNAUTHENTICATED", "访问凭据无效")
                        if not authenticated:
                            authenticated = True
                            sender = subscribe()
                        result = {"authenticated": True}
                    else:
                        if not authenticated:
                            raise RemoteError("UNAUTHENTICATED", "请先发送 auth")
                        result = await self.execute(method, params, peer)
                    await peer.send({"type": "reply", "id": request_id, "result": result})
                except RemoteError as error:
                    await peer.send({"type": "reply", "id": request_id,
                                     "error": error.payload()})
                    if error.code == "UNAUTHENTICATED":
                        await ws.close(code=1008, message="访问凭据无效".encode())
                        break
        except (ConnectionError, asyncio.TimeoutError):
            pass
        finally:
            self.subscribers.discard(peer)
            if sender:
                sender.cancel()
                with suppress(asyncio.CancelledError):
                    await sender
            await self.release_peer(peer)
            self.peers.discard(peer)
        return ws


GATEWAY = web.AppKey("gateway", Gateway)


@web.middleware
async def middleware(request, handler):
    gateway = request.app[GATEWAY]
    origin_allowed = False
    try:
        gateway.check_origin(request)
        origin_allowed = True
        if request.method == "OPTIONS":
            response = web.Response(status=204)
            response.headers["Access-Control-Allow-Methods"] = "GET, POST, PUT, DELETE, OPTIONS"
            response.headers["Access-Control-Allow-Headers"] = "Authorization, Content-Type"
        else:
            if request.path.startswith("/api/") and not gateway.authorized(
                    request.headers.get("Authorization", "")):
                raise RemoteError("UNAUTHENTICATED", "访问凭据无效")
            response = await handler(request)
    except RemoteError as error:
        response = web.json_response({"error": error.payload()}, status=error.status)
    except web.HTTPException as error:
        response = web.json_response({"error": {"code": f"HTTP_{error.status}",
                                                "message": error.reason}}, status=error.status)
    except Exception:
        log.exception("网页请求处理失败")
        response = web.json_response({"error": {"code": "INTERNAL", "message": "请求处理失败"}},
                                     status=500)
    if not response.prepared:
        response.headers["Cache-Control"] = "no-store"
        if response.status == 401:
            response.headers["WWW-Authenticate"] = "Bearer"
        if origin_allowed and "Origin" in request.headers:
            response.headers["Access-Control-Allow-Origin"] = request.headers["Origin"]
            response.headers["Vary"] = "Origin"
    return response


def command_handler(command):
    async def handle(request):
        if request.method == "GET":
            params = {}
        else:
            if request.content_type != "application/json":
                raise RemoteError("INVALID_ARGUMENT", "请求需使用 application/json", 415)
            try:
                body = await request.text()
            except (UnicodeError, LookupError):
                raise RemoteError("INVALID_ARGUMENT", "请求文本编码无效") from None
            params = load_json(body)
        result = await request.app[GATEWAY].execute(command, params)
        return web.json_response({"result": result})
    return handle


async def health(request):
    await request.app[GATEWAY].rpc("GetState", pb.Empty())
    return web.json_response({"ok": True, "robotd": "connected"})


async def lifecycle(app):
    gateway = app[GATEWAY]
    gateway.channel = grpc.aio.insecure_channel(gateway.target, options=[
        ("grpc.enable_http_proxy", 0), ("grpc.max_receive_message_length", MAX_MESSAGE_SIZE),
        ("grpc.max_send_message_length", MAX_MESSAGE_SIZE)])
    gateway.stub = pg.RobotControlStub(gateway.channel)
    watcher = asyncio.create_task(gateway.watch())
    try:
        yield
    finally:
        watcher.cancel()
        with suppress(asyncio.CancelledError):
            await watcher
        for peer in tuple(gateway.peers):
            await gateway.release_peer(peer)
        for session_id, session in list(gateway.sessions.items()):
            gateway.forget(session_id)
            with suppress(RemoteError):
                await gateway.rpc("ReleaseControl", pb.LeaseRequest(token=session.token))
        await gateway.channel.close()


async def shutdown(app):
    gateway = app[GATEWAY]
    gateway.closing = True
    await asyncio.gather(*(peer.ws.close(code=1001, message="服务退出".encode())
                           for peer in tuple(gateway.peers)))


def create_app(target="127.0.0.1:50051", *, robotd_token="", api_token="",
               allowed_origins=(), state_interval_ms=100, rpc_timeout=2, web_root=None):
    if not 50 <= state_interval_ms <= 5000:
        raise ValueError("状态间隔必须在 50..5000 ms")
    if not 0.1 <= rpc_timeout <= 5:
        raise ValueError("RPC 超时必须在 0.1..5 s")
    app = web.Application(middlewares=[middleware], client_max_size=MAX_MESSAGE_SIZE)
    gateway = Gateway(target, robotd_token, api_token, allowed_origins,
                      state_interval_ms, rpc_timeout)
    app[GATEWAY] = gateway
    app.cleanup_ctx.append(lifecycle)
    app.on_shutdown.append(shutdown)
    app.router.add_get("/healthz", health)
    app.router.add_get("/ws", gateway.websocket)
    for method, path, command in ROUTES:
        app.router.add_route(method, path, command_handler(command))
    if web_root:
        root = Path(web_root).resolve(strict=True)
        if not root.is_dir() or not (root / "index.html").is_file():
            raise ValueError("网页目录需包含 index.html")
        async def index(request):
            return web.FileResponse(root / "index.html")
        app.router.add_get("/", index)
        app.router.add_static("/", root, show_index=False)
    return app


def main():
    parser = argparse.ArgumentParser(description="独立的 HTTP/WebSocket robotd 遥控网关")
    parser.add_argument("--robotd", default="127.0.0.1:50051", help="robotd gRPC 地址")
    parser.add_argument("--host", default="127.0.0.1", help="HTTP 监听地址")
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--allow-origin", action="append", default=[], help="允许的网页来源，可重复")
    parser.add_argument("--web-root", type=Path, help="可选：网页遥控器静态目录")
    parser.add_argument("--state-interval-ms", type=int, default=100)
    args = parser.parse_args()
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(message)s")
    if not 1 <= args.port <= 65535:
        parser.error("端口必须在 1..65535")
    try:
        app = create_app(args.robotd, robotd_token=os.environ.get("ROBOTD_API_TOKEN", ""),
                         api_token=os.environ.get("ROBOT_REMOTE_API_TOKEN", ""),
                         allowed_origins=args.allow_origin, web_root=args.web_root,
                         state_interval_ms=args.state_interval_ms)
    except (ValueError, OSError) as error:
        parser.error(str(error))
    log.info("启动网页遥控服务：http://%s:%s，robotd：%s", args.host, args.port, args.robotd)
    web.run_app(app, host=args.host, port=args.port, print=None, shutdown_timeout=3)


if __name__ == "__main__":
    main()
