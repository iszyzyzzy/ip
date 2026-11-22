"""从公网出站连接的车端 WebRTC 服务。"""
import argparse
import asyncio
from collections import deque
from contextlib import suppress
import json
import logging
import os
from pathlib import Path
import secrets
import signal
import sys
from types import SimpleNamespace
import time

from aiohttp import ClientError, ClientSession, WSMsgType
from aiortc import RTCConfiguration, RTCIceServer, RTCPeerConnection, RTCSessionDescription
from aiortc.sdp import candidate_from_sdp
import grpc

from robot_remote import RemoteError
from robot_remote.gamepad import Gamepad
from robot_remote.server import COMMANDS, Gateway, json_object, load_json, message_dict
from robotd.pb import robot_pb2 as pb, robot_pb2_grpc as pg

from .media import Camera, Talk

log = logging.getLogger(__name__)
CHANNELS = {"rpc.v1": True, "drive.v1": False, "state.v1": False, "stop.v1": True}


def tick_ms():
    return time.monotonic_ns() // 1_000_000


class LinkSession:
    def __init__(self, sid, ice_servers, gateway, args):
        servers = [RTCIceServer(**server) for server in ice_servers]
        self.pc = RTCPeerConnection(RTCConfiguration(iceServers=servers))
        self.pc.on("datachannel", self.on_channel)
        self.pc.on("track", self.on_track)
        self.pc.on("connectionstatechange", self.on_connection)
        self.sid, self.gateway, self.args = sid, gateway, args
        self.peer = SimpleNamespace(session_id="")
        self.channels = {}
        self.epoch = ""
        self.highest_seq = 0
        self.ticks = deque(maxlen=10)
        self.latest_drive = None
        self.drive_ready = asyncio.Event()
        self.rpc_queue = asyncio.Queue(maxsize=16)
        self.stop_queue = asyncio.Queue(maxsize=1)
        self.closed = asyncio.Event()
        self.cleanup = None
        self.stopping = False
        self.generation = 0
        self.dropped_drive = 0
        self.camera = None
        self.talk = Talk(gateway)
        self.tasks = [asyncio.create_task(task()) for task in (
            self.watch_state, self.drive_loop, self.rpc_loop, self.stop_loop)]

    def on_connection(self):
        if self.pc.connectionState in {"failed", "closed"}:
            self.closed.set()

    def on_track(self, track):
        if track.kind == "audio" and not self.talk.track:
            self.talk.attach(track)

    def on_channel(self, channel):
        label = channel.label
        reliable = CHANNELS.get(label)
        if (reliable is None or label in self.channels or channel.ordered != reliable
                or channel.maxRetransmits != (None if reliable else 0)
                or channel.maxPacketLifeTime is not None):
            channel.close()
            return
        self.channels[label] = channel
        if label != "state.v1":
            channel.on("close", self.closed.set)
            channel.on("message", lambda raw: self.receive(label, raw))

    def send(self, label, msg):
        channel = self.channels.get(label)
        if channel and channel.readyState == "open" and channel.bufferedAmount < 128 * 1024:
            channel.send(json.dumps(msg, ensure_ascii=False, allow_nan=False))

    def receive(self, label, raw):
        if self.closed.is_set() or not isinstance(raw, str) or len(raw) > 60_000:
            return
        try:
            msg = load_json(raw)
        except RemoteError:
            return
        if label == "drive.v1":
            seq = msg.get("seq")
            if (self.stopping or not self.peer.session_id or msg.get("epoch") != self.epoch
                    or self.tick_age(msg.get("tick")) > 200
                    or type(seq) is not int or seq <= self.highest_seq):
                self.dropped_drive += 1
                return
            self.highest_seq = seq
            self.latest_drive = msg
            self.drive_ready.set()
        elif label == "stop.v1":
            if msg.get("method") == "stop" and not self.stop_queue.full():
                self.stop_queue.put_nowait(msg)
        elif not self.rpc_queue.full():
            self.rpc_queue.put_nowait((self.generation, msg))
        else:
            self.send(label, {"type": "reply", "id": msg.get("id"),
                              "error": {"code": "BUSY", "message": "命令队列已满"}})

    def tick_age(self, tick):
        if type(tick) is int and tick in self.ticks:
            return tick_ms() - tick
        return 10_000

    async def answer(self, sdp):
        if not isinstance(sdp, str) or self.pc.remoteDescription is not None:
            raise ValueError("每个会话须发送一个 SDP offer")
        await self.pc.setRemoteDescription(RTCSessionDescription(sdp, "offer"))
        if self.args.camera and any(t.kind == "video" for t in self.pc.getTransceivers()):
            self.camera = Camera(self.args.camera, self.args.camera_format, self.args.ffmpeg)
            self.pc.addTrack(self.camera)
        await self.pc.setLocalDescription(await self.pc.createAnswer())
        return {"type": "answer", "sid": self.sid, "sdp": self.pc.localDescription.sdp}

    async def add_ice(self, data):
        if data is None:
            await self.pc.addIceCandidate(None)
            return
        candidate = candidate_from_sdp(data["candidate"].removeprefix("candidate:"))
        candidate.sdpMid = data.get("sdpMid")
        candidate.sdpMLineIndex = data.get("sdpMLineIndex")
        await self.pc.addIceCandidate(candidate)

    async def watch_state(self):
        seq = 0
        while True:
            try:
                state = message_dict(await self.gateway.rpc("GetState", pb.Empty()))
                status = {"state": state}
            except RemoteError as error:
                status = {"error": error.payload()}
            self.gateway.expire_sessions()
            channel = self.channels.get("state.v1")
            if channel and channel.readyState == "open" and channel.bufferedAmount == 0:
                seq += 1
                tick = tick_ms()
                self.ticks.append(tick)
                self.send("state.v1", {"seq": seq, "tick": tick, **status,
                          "epoch": self.epoch if self.peer.session_id else "",
                          "video": self.camera.status if self.camera else "inactive",
                          "talk": self.talk.status, "dropped_drive": self.dropped_drive})
            await asyncio.sleep(.1)

    async def drive_loop(self):
        while True:
            await self.drive_ready.wait()
            self.drive_ready.clear()
            msg = self.latest_drive
            age = self.tick_age(msg.get("tick"))
            if (self.stopping or not self.peer.session_id or msg.get("epoch") != self.epoch
                    or age > 200):
                self.dropped_drive += 1
                continue
            try:
                chassis = dict(json_object(msg.get("chassis")))
                chassis["ttl_ms"] = min(200, 300 - age)
                async with asyncio.timeout(.08):
                    await self.gateway.execute("targets", {"targets": {"chassis": chassis}}, self.peer)
            except (RemoteError, asyncio.TimeoutError) as error:
                self.dropped_drive += 1
                log.debug("底盘命令未执行：%s", error)

    async def call(self, method, params, generation):
        if method in {"get_gamepad", "set_gamepad"}:
            return await self.gateway.execute(method, params)
        if method in {"talk.begin", "talk.end"}:
            if method == "talk.begin":
                if self.stopping or generation != self.generation:
                    raise RemoteError("FAILED_PRECONDITION", "急停已中断喊话")
                return await self.talk.begin()
            return self.talk.end()
        if method not in COMMANDS or method == "stop":
            raise RemoteError("NOT_FOUND", "未知命令")
        params.pop("session_id", None)
        epoch = params.pop("epoch", None)
        tick = params.pop("tick", None)
        _, _, needs_lease = COMMANDS[method]
        if method == "acquire" or needs_lease:
            if self.stopping or generation != self.generation:
                raise RemoteError("FAILED_PRECONDITION", "急停已中断命令")
            if method != "release" and self.tick_age(tick) > 500:
                raise RemoteError("STALE", "请使用最近的车端 tick")
            if needs_lease and (not self.peer.session_id or epoch != self.epoch):
                raise RemoteError("PERMISSION_DENIED", "接管已失效")
        if method == "targets" and "chassis" in json_object(params.get("targets")):
            raise RemoteError("INVALID_ARGUMENT", "底盘目标使用 drive.v1")
        if method == "acquire":
            params.update(owner="wan/desktop", ttl_ms=1000)
        result = await self.gateway.execute(method, params, self.peer)
        if method == "acquire":
            if self.stopping or generation != self.generation or self.closed.is_set():
                await self.gateway.release_peer(self.peer)
                raise RemoteError("FAILED_PRECONDITION", "接管已中断")
            self.epoch = secrets.token_urlsafe(12)
            self.highest_seq = 0
        elif method == "release":
            self.epoch = ""
        if method in {"acquire", "renew"}:
            return {"epoch": self.epoch, "ttl_ms": result["ttl_ms"]}
        return result

    async def rpc_loop(self):
        while True:
            generation, msg = await self.rpc_queue.get()
            try:
                method = msg.get("method")
                if not isinstance(method, str):
                    raise RemoteError("INVALID_ARGUMENT", "缺少 method")
                result = await self.call(method, json_object(msg.get("params", {})), generation)
                reply = {"result": result}
            except RemoteError as error:
                reply = {"error": error.payload()}
            self.send("rpc.v1", {"type": "reply", "id": msg.get("id"), **reply})

    async def stop_loop(self):
        while True:
            msg = await self.stop_queue.get()
            self.stopping = True
            self.generation += 1
            self.epoch = ""
            self.talk.abort()
            try:
                reply = {"result": await self.gateway.execute("stop", {"latch": True})}
            except RemoteError as error:
                reply = {"error": error.payload()}
                await self.gateway.release_peer(self.peer)
            finally:
                self.stopping = False
            self.send("stop.v1", {"type": "reply", "id": msg.get("id"), **reply})

    async def close(self):
        # 信令断开与进程退出可能同时触发清理，不能中途取消 aiortc.close。
        if self.cleanup is None:
            self.cleanup = asyncio.create_task(self.shutdown())
        await asyncio.shield(self.cleanup)

    async def shutdown(self):
        self.closed.set()
        for task in self.tasks:
            task.cancel()
        await asyncio.gather(*self.tasks, return_exceptions=True)
        await self.gateway.release_peer(self.peer)
        await self.talk.close()
        await self.pc.close()
        if self.camera:
            await self.camera.close()


async def serve_connection(http, gateway, args, token):
    async with http.ws_connect(args.signal, headers={"Authorization": f"Bearer {token}"},
                               heartbeat=5) as socket:
        await socket.send_json({"type": "robot.hello", "robot_id": args.robot_id, "version": 1,
                                "capabilities": {"audio": True, "video": bool(args.camera)}})
        session = None
        try:
            while True:
                incoming = asyncio.create_task(socket.receive())
                ended = asyncio.create_task(session.closed.wait()) if session else None
                tasks = {incoming, ended} if ended else {incoming}
                try:
                    done, _ = await asyncio.wait(tasks, return_when=asyncio.FIRST_COMPLETED)
                finally:
                    for task in tasks:
                        if not task.done():
                            task.cancel()
                    await asyncio.gather(*tasks, return_exceptions=True)
                if ended and ended in done:
                    sid = session.sid
                    await session.close()
                    session = None
                    await socket.send_json({"type": "session.close", "sid": sid})
                if incoming not in done:
                    continue
                frame = incoming.result()
                if frame.type != WSMsgType.TEXT:
                    return
                msg = frame.json()
                kind, sid = msg["type"], msg.get("sid")
                if kind == "robot.ready":
                    log.info("车辆 %s 已上线", args.robot_id)
                elif kind == "session.opened":
                    if session:
                        await session.close()
                    session = LinkSession(sid, msg["ice_servers"], gateway, args)
                elif session and sid == session.sid:
                    try:
                        if kind == "offer":
                            await socket.send_json(await asyncio.wait_for(session.answer(msg["sdp"]), 20))
                        elif kind == "ice":
                            await session.add_ice(msg.get("candidate"))
                        elif kind == "session.closed":
                            await session.close()
                            session = None
                    except (ValueError, TypeError, KeyError, IndexError, asyncio.TimeoutError) as error:
                        log.warning("协商失败：%s", error)
                        await session.close()
                        session = None
                        await socket.send_json({"type": "session.close", "sid": sid})
        finally:
            if session:
                await session.close()


async def run(args, token):
    gateway = Gateway(args.robotd, robotd_token=os.getenv("ROBOTD_API_TOKEN", ""), rpc_timeout=.5)
    state_dir = Path(os.getenv("STATE_DIRECTORY") or Path.home() / ".config/robot-remote")
    gateway.gamepad = Gamepad(gateway, state_dir / "gamepad.json")
    async with grpc.aio.insecure_channel(args.robotd, options=[("grpc.enable_http_proxy", 0)]) as channel:
        gateway.stub = pg.RobotControlStub(channel)
        gamepad = asyncio.create_task(gateway.gamepad.run())
        try:
            async with ClientSession() as http:
                while True:
                    try:
                        await serve_connection(http, gateway, args, token)
                    except (ClientError, OSError, asyncio.TimeoutError) as error:
                        log.warning("公网信令断开：%s", error)
                    await asyncio.sleep(2)
        finally:
            gamepad.cancel()
            with suppress(asyncio.CancelledError):
                await gamepad


async def run_service(args, token):
    task = asyncio.create_task(run(args, token))
    if sys.platform != "win32":
        asyncio.get_running_loop().add_signal_handler(signal.SIGTERM, task.cancel)
    with suppress(asyncio.CancelledError):
        await task


def main():
    parser = argparse.ArgumentParser(description="车端 WebRTC 遥控服务")
    parser.add_argument("--signal", required=True)
    parser.add_argument("--robot-id", default="car-01")
    parser.add_argument("--robotd", default="127.0.0.1:50051")
    parser.add_argument("--camera", help="车载 V4L2 设备路径")
    parser.add_argument("--camera-format", choices=["v4l2", "lavfi"], default="v4l2")
    parser.add_argument("--ffmpeg", default="ffmpeg")
    args = parser.parse_args()
    token = os.getenv("WAN_ROBOT_TOKEN")
    if not token:
        parser.error("请设置 WAN_ROBOT_TOKEN")
    logging.basicConfig(level=logging.INFO)
    with suppress(KeyboardInterrupt):
        asyncio.run(run_service(args, token))


if __name__ == "__main__":
    main()
