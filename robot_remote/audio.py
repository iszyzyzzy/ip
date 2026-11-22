import asyncio
from array import array
import sys
import wave

import grpc
from aiohttp import WSMsgType, web
from robotd.pb import robot_pb2 as pb

from . import RemoteError


def register_audio(app, gateway, directory):
    setting = directory.parent / "volume.txt"
    try:
        volume = max(0, min(100, int(setting.read_text())))
    except (FileNotFoundError, ValueError):
        volume = 100

    async def get_volume(request):
        return web.json_response(volume)

    async def set_volume(request):
        nonlocal volume
        try:
            value = await request.json()
        except ValueError:
            value = None
        if type(value) is not int or not 0 <= value <= 100:
            raise RemoteError("INVALID_ARGUMENT", "音量需为 0 到 100 的整数")
        setting.parent.mkdir(parents=True, exist_ok=True)
        tmp = setting.with_suffix(".tmp")
        tmp.write_text(str(value))
        tmp.replace(setting)
        volume = value
        return web.json_response(volume)

    def adjust(data):
        if volume == 100:
            return data
        if volume == 0:
            return bytes(len(data))
        samples = array("h")
        samples.frombytes(data)
        if sys.byteorder != "little":
            samples.byteswap()
        gain = volume / 100
        samples = array("h", (round(s * gain) for s in samples))
        if sys.byteorder != "little":
            samples.byteswap()
        return samples.tobytes()

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
                    # 只提前发送一小段，播放中调音量能及时生效。
                    clock = asyncio.get_running_loop().time
                    start = clock()
                    while data := wav.readframes(1024):
                        await asyncio.sleep(max(0, start + wav.tell() / wav.getframerate() - 0.12 - clock()))
                        yield pb.AudioPacket(pcm=adjust(data))

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
                    if len(msg.data) % 2:
                        raise ValueError("PCM16 采样不完整")
                    await call.write(pb.AudioPacket(pcm=adjust(msg.data)))
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
    app.router.add_get("/api/audio/volume", get_volume)
    app.router.add_put("/api/audio/volume", set_volume)
    app.router.add_post("/api/audio/presets/{name}", play)
    app.router.add_get("/talk", talk)
