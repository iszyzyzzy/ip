import asyncio
from contextlib import AsyncExitStack
import json
import os
import shutil
from types import SimpleNamespace
import unittest

from aiohttp import ClientSession, WSServerHandshakeError
from aiohttp.test_utils import TestServer
from aiortc import AudioStreamTrack, RTCConfiguration, RTCPeerConnection, RTCSessionDescription
import grpc

from robot_remote.server import Gateway
from robotd.pb import robot_pb2 as pb, robot_pb2_grpc as pg
from robotd.tests.support import RustDaemon
from robot_wan.link import CHANNELS, serve_connection
from robot_wan.signal import Signal, create_app


FFMPEG = os.getenv("FFMPEG") or shutil.which("ffmpeg")


class WanTests(unittest.IsolatedAsyncioTestCase):
    @unittest.skipUnless(FFMPEG, "需要 FFmpeg 生成测试视频")
    async def test_webrtc_control_media_and_disconnect(self):
        daemon = await asyncio.to_thread(RustDaemon)
        self.addAsyncCleanup(asyncio.to_thread, daemon.close)
        target = f"127.0.0.1:{daemon.port}"
        signal = Signal("car-01", "robot", "operator", [], [], None)
        async with AsyncExitStack() as stack:
            grpc_channel = await stack.enter_async_context(grpc.aio.insecure_channel(target))
            gateway = Gateway(target)
            gateway.stub = pg.RobotControlStub(grpc_channel)
            config = await gateway.execute("get_config", {})
            config["config"]["audio"]["enabled"] = True
            await gateway.execute("update_config", {"expected_revision": config["revision"],
                                                    "config": config["config"]})
            server = await stack.enter_async_context(TestServer(create_app(signal)))
            url = str(server.make_url("/v1/signal"))
            http = await stack.enter_async_context(ClientSession())
            with self.assertRaises(WSServerHandshakeError) as denied:
                await http.ws_connect(url)
            self.assertEqual(denied.exception.status, 401)
            args = SimpleNamespace(signal=url, robot_id="car-01", camera="testsrc=size=640x480:rate=15",
                                   camera_format="lavfi", ffmpeg=FFMPEG)
            vehicle = asyncio.create_task(serve_connection(http, gateway, args, "robot"))

            async def stop_vehicle():
                vehicle.cancel()
                await asyncio.gather(vehicle, return_exceptions=True)

            stack.push_async_callback(stop_vehicle)
            ws = await stack.enter_async_context(
                http.ws_connect(url, headers={"Authorization": "Bearer operator"}))
            async with asyncio.timeout(5):
                while signal.robot is None:
                    await asyncio.sleep(.02)
            await ws.send_json({"type": "session.open", "robot_id": "car-01", "version": 1})
            opened = await ws.receive_json(timeout=3)
            self.assertEqual(opened["type"], "session.opened")
            await ws.send_json({"type": "session.open", "robot_id": "car-01", "version": 1})
            self.assertEqual((await ws.receive_json(timeout=3))["error"]["code"], "BUSY")

            pc = RTCPeerConnection(RTCConfiguration(iceServers=[]))
            stack.push_async_callback(pc.close)
            received_video = asyncio.Queue()
            pc.on("track", received_video.put_nowait)
            pc.addTransceiver(AudioStreamTrack(), direction="sendonly")
            pc.addTransceiver("video", direction="recvonly")
            channels, messages = {}, {}
            for label, ordered in CHANNELS.items():
                channel = pc.createDataChannel(label, ordered=ordered,
                                               maxRetransmits=None if ordered else 0)
                messages[label] = asyncio.Queue()
                channel.on("message", messages[label].put_nowait)
                channels[label] = channel

            await pc.setLocalDescription(await pc.createOffer())
            await ws.send_json({"type": "offer", "sid": opened["sid"],
                                "sdp": pc.localDescription.sdp})
            answer = await ws.receive_json(timeout=10)
            await pc.setRemoteDescription(RTCSessionDescription(answer["sdp"], "answer"))
            track = await asyncio.wait_for(received_video.get(), 5)
            frame = await asyncio.wait_for(track.recv(), 8)
            self.assertEqual((frame.width, frame.height), (640, 480))

            async def message(label, predicate=lambda _: True):
                async with asyncio.timeout(4):
                    while True:
                        msg = json.loads(await messages[label].get())
                        if predicate(msg):
                            return msg

            next_id = 0

            async def command(method, params, label="rpc.v1"):
                nonlocal next_id
                next_id += 1
                channels[label].send(json.dumps({"method": method, "params": params, "id": next_id}))
                return await message(label, lambda msg: msg["id"] == next_id)

            state = await message("state.v1", lambda msg: msg["video"] == "live")
            self.assertEqual(state["state"]["owner"], "")
            self.assertEqual((await command("acquire", {"tick": 0}))["error"]["code"], "STALE")
            state = await message("state.v1", lambda msg: msg["seq"] > state["seq"])
            lease = await command("acquire", {"tick": state["tick"]})
            epoch = lease["result"]["epoch"]
            channels["drive.v1"].send(json.dumps({"epoch": epoch, "seq": 1, "tick": state["tick"],
                                                "chassis": {"speed_m_s": .1}}))

            async def robot_until(predicate):
                async with asyncio.timeout(4):
                    while True:
                        s = await gateway.execute("get_state", {})
                        if predicate(s):
                            return s
                        await asyncio.sleep(.02)

            await robot_until(lambda s: not s["stopped"])
            self.assertIn("result", await command("stop", {}, "stop.v1"))
            await robot_until(lambda s: s["latched"] and s["stopped"] and not s["owner"])
            state = await message("state.v1", lambda msg: msg["state"]["latched"])
            self.assertIn("result", await command("acquire", {"tick": state["tick"], "clear_latch": True}))
            self.assertIn("result", await command("talk.begin", {}))
            async with asyncio.timeout(4):
                while (await gateway.rpc("GetAudioState", pb.Empty())).received_frames == 0:
                    await asyncio.sleep(.025)
            self.assertIn("result", await command("talk.end", {}))
            await message("state.v1", lambda msg: msg["talk"]["phase"] == "completed")
            state = await message("state.v1", lambda msg: msg["epoch"])
            renewed = await command("renew", {"tick": state["tick"], "epoch": state["epoch"]})
            self.assertIn("result", renewed)
            self.assertEqual((await gateway.execute("get_state", {}))["owner"], "wan/desktop")
            await ws.close()
            await robot_until(lambda s: not s["owner"] and s["stopped"])
            self.assertIsNone(signal.session)
