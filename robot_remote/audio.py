import asyncio
import wave

import grpc
from aiohttp import WSMsgType, web
from robotd.pb import robot_pb2 as pb

from . import RemoteError


def register_audio(app, gateway, directory):
    async def presets(request):
        return web.json_response(sorted(p.stem for p in directory.glob("*.wav") if p.is_file()))

    async def play(request):
        name = request.match_info["name"]
        if "/" in name or "\\" in name:
            raise web.HTTPNotFound()
        try:
            with wave.open(str(directory / (name + ".wav")), "rb") as wav:
                if wav.getsampwidth() != 2:
                    raise RemoteError("INVALID_ARGUMENT", "音频需为 PCM16 WAV")

                async def packets():
                    yield pb.AudioPacket(header=pb.AudioHeader(
                        sample_rate_hz=wav.getframerate(), channels=wav.getnchannels()))
                    while data := wav.readframes(1024):
                        yield pb.AudioPacket(pcm=data)

                result = await gateway.stub.PlayAudio(packets(), metadata=gateway.metadata)
                return web.json_response({"phase": result.phase, "frames": result.submitted_frames})
        except FileNotFoundError:
            raise web.HTTPNotFound()
        except (wave.Error, EOFError):
            raise RemoteError("INVALID_ARGUMENT", "无法读取 WAV 文件") from None
        except grpc.aio.AioRpcError as error:
            raise RemoteError(error.code().name, error.details()) from None

    async def talk(request):
        ws = web.WebSocketResponse(max_msg_size=65536, heartbeat=10)
        await ws.prepare(request)
        call = None
        try:
            hello = await ws.receive_json(timeout=5)
            if not isinstance(hello, dict) or not gateway.authorized("Bearer " + str(hello.get("token", ""))):
                raise ValueError("访问凭据无效")
            call = gateway.stub.PlayAudio(metadata=gateway.metadata)
            await call.write(pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=48000, channels=1)))
            await ws.send_str("ready")
            async for msg in ws:
                if msg.type == WSMsgType.BINARY:
                    await call.write(pb.AudioPacket(pcm=msg.data))
                elif msg.type == WSMsgType.TEXT and msg.data == "end":
                    await call.done_writing()
                    result = await call
                    await ws.send_str(result.phase)
                    break
                else:
                    break
        except (grpc.RpcError, asyncio.InvalidStateError, ValueError, TypeError, asyncio.TimeoutError) as error:
            if not ws.closed:
                await ws.send_json({"error": error.details() if isinstance(error, grpc.aio.AioRpcError) else str(error)})
        finally:
            if call:
                call.cancel()
            await ws.close()
        return ws

    app.router.add_get("/api/audio/presets", presets)
    app.router.add_post("/api/audio/presets/{name}", play)
    app.router.add_get("/talk", talk)
