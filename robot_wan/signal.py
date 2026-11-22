"""转发单车 WebRTC 信令；连接归属由预配置的两份凭据确定。"""
import argparse
import base64
from contextlib import suppress
import hmac
import os
import secrets
import time

from aiohttp import WSMsgType, web

from robot_remote import RemoteError
from robot_remote.server import load_json


class Signal:
    def __init__(self, robot_id, robot_token, operator_token, stun, turn, turn_secret):
        if not robot_token or not operator_token or robot_token == operator_token:
            raise ValueError("车辆和操作员须使用不同的非空凭据")
        if bool(turn) != bool(turn_secret):
            raise ValueError("TURN 地址与共享密钥须一起配置")
        self.robot_id = robot_id
        self.tokens = {"robot": robot_token, "operator": operator_token}
        self.stun = stun
        self.turn = turn
        self.turn_secret = turn_secret
        self.robot = None
        self.capabilities = {}
        self.session = None
        self.sockets = set()

    def ice_servers(self, role):
        servers = [{"urls": list(self.stun)}] if self.stun else []
        if self.turn:
            # coturn 的 REST 凭据以用户名前缀表示 Unix 到期时间。
            username = f"{int(time.time()) + 43200}:{role}"
            digest = hmac.digest(self.turn_secret.encode(), username.encode(), "sha1")
            servers.append({"urls": list(self.turn), "username": username,
                            "credential": base64.b64encode(digest).decode()})
        return servers

    async def close_session(self, reason):
        if self.session:
            sid, robot, operator = self.session
            self.session = None
            for socket in (robot, operator):
                with suppress(ConnectionError):
                    if not socket.closed:
                        await socket.send_json({"type": "session.closed", "sid": sid, "reason": reason})

    async def message(self, socket, role, msg):
        kind = msg.get("type")
        if kind == "robot.hello" and role == "robot":
            if msg.get("robot_id") != self.robot_id or msg.get("version") != 1:
                raise RemoteError("INVALID_ARGUMENT", "车辆 ID 或协议版本不匹配")
            if self.robot is not None and self.robot is not socket:
                raise RemoteError("BUSY", "车辆连接已存在")
            self.robot = socket
            caps = msg.get("capabilities", {})
            self.capabilities = {key: isinstance(caps, dict) and caps.get(key) is True
                                 for key in ("audio", "video")}
            return {"type": "robot.ready", "robot_id": self.robot_id, "version": 1}
        if kind == "robots.list" and role == "operator":
            return {"type": "robots.list", "robots": [{"robot_id": self.robot_id,
                    "online": self.robot is not None, "busy": self.session is not None,
                    "capabilities": self.capabilities if self.robot else {}}]}
        if kind == "session.open" and role == "operator":
            if msg.get("robot_id") != self.robot_id or msg.get("version") != 1:
                raise RemoteError("INVALID_ARGUMENT", "车辆 ID 或协议版本不匹配")
            if not self.robot or self.robot.closed:
                raise RemoteError("ROBOT_OFFLINE", "车辆未上线")
            if self.session:
                raise RemoteError("BUSY", "车辆已有远程会话")
            sid = secrets.token_urlsafe(16)
            robot = self.robot
            self.session = (sid, robot, socket)
            common = {"type": "session.opened", "sid": sid, "robot_id": self.robot_id,
                      "capabilities": self.capabilities}
            try:
                await robot.send_json({**common, "ice_servers": self.ice_servers("robot")})
            except ConnectionError:
                self.session = None
                raise RemoteError("ROBOT_OFFLINE", "车辆已断开") from None
            return {**common, "ice_servers": self.ice_servers("operator")}
        if kind not in ("offer", "answer", "ice", "session.close"):
            raise RemoteError("INVALID_ARGUMENT", "未知信令类型")
        session = self.session
        if not session or msg.get("sid") != session[0] or socket not in session[1:]:
            raise RemoteError("NOT_FOUND", "会话不存在")
        sid, robot, operator = session
        if kind == "session.close":
            await self.close_session("closed")
            return None
        if (kind == "offer" and role != "operator") or (kind == "answer" and role != "robot"):
            raise RemoteError("INVALID_ARGUMENT", "SDP 发送方向错误")
        field = "candidate" if kind == "ice" else "sdp"
        target = operator if socket is robot else robot
        await target.send_json({"type": kind, "sid": sid, field: msg.get(field)})
        return None

    async def websocket(self, request):
        auth = request.headers.get("Authorization", "").encode()
        role = next((role for role, token in self.tokens.items()
                     if hmac.compare_digest(auth, f"Bearer {token}".encode())), None)
        if role is None:
            raise web.HTTPUnauthorized()
        socket = web.WebSocketResponse(heartbeat=5, max_msg_size=128 * 1024)
        await socket.prepare(request)
        self.sockets.add(socket)
        try:
            async for frame in socket:
                if frame.type != WSMsgType.TEXT:
                    break
                msg = {}
                try:
                    msg = load_json(frame.data)
                    reply = await self.message(socket, role, msg)
                    if reply is None:
                        continue
                except RemoteError as error:
                    reply = {"type": "error", "error": error.payload()}
                except ConnectionError:
                    await self.close_session("peer_disconnected")
                    break
                await socket.send_json({**reply, "id": msg.get("id")})
        finally:
            self.sockets.discard(socket)
            if self.robot is socket:
                self.robot = None
            if self.session and socket in self.session[1:]:
                await self.close_session("peer_disconnected")
        return socket

    async def shutdown(self, _app):
        for socket in tuple(self.sockets):
            await socket.close(code=1001)


def create_app(state):
    app = web.Application()

    async def health(_request):
        return web.json_response({"ok": True})

    app.router.add_get("/healthz", health)
    app.router.add_get("/v1/signal", state.websocket)
    app.on_shutdown.append(state.shutdown)
    return app


def main():
    parser = argparse.ArgumentParser(description="公网 WebRTC 信令服务")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--robot-id", default="car-01")
    parser.add_argument("--stun", action="append", default=[])
    parser.add_argument("--turn", help="本实例提供的 TURN URL")
    args = parser.parse_args()
    try:
        state = Signal(args.robot_id, os.getenv("WAN_ROBOT_TOKEN"),
                       os.getenv("WAN_OPERATOR_TOKEN"), args.stun, [args.turn] if args.turn else [],
                       os.getenv("WAN_TURN_SECRET"))
    except ValueError as error:
        parser.error(str(error))
    web.run_app(create_app(state), host=args.host, port=args.port)


if __name__ == "__main__":
    main()
