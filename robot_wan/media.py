"""车端摄像头和实时喊话适配。"""
import asyncio
from contextlib import suppress
import fractions
import logging
import time

from aiortc import VideoStreamTrack
from aiortc.mediastreams import MediaStreamError
from av import AudioFifo, AudioResampler, VideoFrame
import grpc

from robot_remote import RemoteError
from robotd.pb import robot_pb2 as pb

log = logging.getLogger(__name__)


class Camera(VideoStreamTrack):
    width, height, fps = 640, 480, 15

    def __init__(self, source, source_format, ffmpeg):
        super().__init__()
        self.status = "starting"
        self.frames = asyncio.Queue(maxsize=1)
        self.worker = asyncio.create_task(self.capture(source, source_format, ffmpeg))

    async def capture(self, source, source_format, ffmpeg):
        size = self.width * self.height * 3 // 2
        while True:
            process = None
            try:
                options = (["-framerate", str(self.fps), "-video_size",
                            f"{self.width}x{self.height}"] if source_format == "v4l2" else ["-re"])
                process = await asyncio.create_subprocess_exec(
                    ffmpeg, "-nostdin", "-loglevel", "error", "-f", source_format,
                    *options, "-i", source, "-an", "-vf", f"scale={self.width}:{self.height}",
                    "-r", str(self.fps), "-pix_fmt", "yuv420p", "-c:v", "rawvideo",
                    "-threads", "1", "-f", "rawvideo", "pipe:1",
                    stdout=asyncio.subprocess.PIPE, limit=size,
                )
                while True:
                    raw = await asyncio.wait_for(process.stdout.readexactly(size), 3)
                    self.publish(raw)
                    self.status = "live"
            except (OSError, EOFError, asyncio.TimeoutError) as error:
                if self.status != "offline":
                    log.warning("摄像头采集失败：%s", error)
                self.status = "offline"
                self.publish(None)
            finally:
                if process is not None:
                    with suppress(ProcessLookupError):
                        process.kill()
                    await process.communicate()
            await asyncio.sleep(1)

    def publish(self, frame):
        if self.frames.full():
            self.frames.get_nowait()
        self.frames.put_nowait(frame)

    async def recv(self):
        if self.readyState != "live":
            raise MediaStreamError
        raw = await self.frames.get()
        while raw is None:
            raw = await self.frames.get()
        frame = VideoFrame(self.width, self.height, "yuv420p")
        offset = 0
        for plane in frame.planes:
            size = plane.width * plane.height
            plane.update(raw[offset:offset + size])
            offset += size
        frame.pts = time.monotonic_ns() // 1000
        frame.time_base = fractions.Fraction(1, 1_000_000)
        return frame

    async def close(self):
        self.stop()
        self.worker.cancel()
        with suppress(asyncio.CancelledError):
            await self.worker


class Talk:
    def __init__(self, gateway):
        self.gateway = gateway
        self.track = None
        self.reader = self.player = None
        self.queue = None
        self.status = {"phase": "idle"}
        self.generation = 0

    def attach(self, track):
        self.track = track
        # RTP 一直排空，打开喊话时只使用新收到的麦克风数据。
        self.reader = asyncio.create_task(self.receive())

    async def receive(self):
        queue = None
        try:
            while True:
                frame = await self.track.recv()
                if self.queue is None:
                    continue
                if queue is not self.queue:
                    queue = self.queue
                    resampler = AudioResampler(format="s16", layout="mono", rate=48000)
                    fifo = AudioFifo()
                for converted in resampler.resample(frame):
                    converted.pts = None
                    fifo.write(converted)
                while fifo.samples >= 960:
                    block = bytes(fifo.read(960).planes[0])[:1920]
                    if queue.full():
                        queue.get_nowait()
                    queue.put_nowait(block)
        except MediaStreamError:
            self.abort()
            self.status = {"phase": "offline"}

    async def begin(self):
        if not self.track or self.track.readyState != "live":
            raise RemoteError("AUDIO_OFFLINE", "会话没有可用的麦克风音轨")
        if self.player and not self.player.done():
            raise RemoteError("AUDIO_BUSY", "上一段喊话仍在播放")
        generation = self.generation
        status = await self.gateway.rpc("GetAudioState", pb.Empty())
        if generation != self.generation:
            raise RemoteError("FAILED_PRECONDITION", "喊话已取消")
        if status.phase == "disabled":
            raise RemoteError("AUDIO_OFFLINE", "robotd 尚未启用音频")
        if status.phase in {"buffering", "playing", "rebuffering", "draining", "stopping"}:
            raise RemoteError("AUDIO_BUSY", "车载播放器正忙")
        self.queue = asyncio.Queue(maxsize=3)
        self.status = {"phase": "accepted"}
        self.player = asyncio.create_task(self.play(self.queue))
        return self.status

    def end(self):
        if self.queue is not None:
            queue, self.queue = self.queue, None
            if queue.full():
                queue.get_nowait()
            queue.put_nowait(None)
        return {"phase": self.status["phase"]}

    async def play(self, queue):
        async def packets():
            yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=48000, channels=1))
            while True:
                block = await asyncio.wait_for(queue.get(), timeout=2)
                if block is None:
                    return
                yield pb.AudioPacket(pcm=block)

        # 无音轨数据时主动取消 RPC，避免旧喊话占用声卡。
        call = self.gateway.stub.PlayAudio(metadata=self.gateway.metadata)
        try:
            async with asyncio.timeout(2) as deadline:
                async for packet in packets():
                    deadline.reschedule(asyncio.get_running_loop().time() + 2)
                    await call.write(packet)
                await call.done_writing()
                result = await call
                self.status = {"phase": result.phase}
        except grpc.aio.AioRpcError as error:
            code = error.code().name
            if code == "RESOURCE_EXHAUSTED":
                code = "AUDIO_BUSY"
            self.status = {"phase": "failed", "code": code, "message": error.details()}
        except asyncio.TimeoutError:
            self.status = {"phase": "failed", "code": "AUDIO_TIMEOUT", "message": "喊话音频超时"}
        finally:
            call.cancel()
            self.queue = None

    def abort(self):
        self.generation += 1
        self.queue = None
        if self.player:
            self.player.cancel()
        self.status = {"phase": "idle"}

    async def close(self):
        self.abort()
        if self.reader:
            self.reader.cancel()
        for task in (self.reader, self.player):
            if task:
                with suppress(asyncio.CancelledError):
                    await task
