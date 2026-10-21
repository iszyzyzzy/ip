"""音频调试与 GPT-SoVITS 流转发；服务端不加载 AI 模型。"""
from __future__ import annotations

import argparse
import json
import os
import sys
import wave
from pathlib import Path
from urllib.request import Request, urlopen

import grpc

from .client import show
from .pb import robot_pb2 as pb
from .pb import robot_pb2_grpc as pg


def pcm_packets(source, rate, channels):
    yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=rate, channels=channels))
    # read1 不等待拼满缓冲，让上游已生成的短片段尽早进入播放队列。
    read = getattr(source, "read1", source.read)
    while data := read(4096):
        yield pb.AudioPacket(pcm=data)


def tts_packets(url, payload, rate):
    yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=rate, channels=1))
    payload = dict(payload, media_type="raw")
    payload["streaming_mode"] = payload.get("streaming_mode") or True
    request = Request(url, json.dumps(payload).encode("utf-8"),
                      headers={"Content-Type": "application/json"})
    with urlopen(request, timeout=60) as response:
        content_type = response.headers.get_content_type()
        if content_type not in ("audio/raw", "application/octet-stream"):
            raise ValueError(f"GPT-SoVITS 未返回 raw PCM：{content_type}")
        while data := response.read1(4096):
            yield pb.AudioPacket(pcm=data)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--target", default="127.0.0.1:50051")
    sub = p.add_subparsers(dest="command", required=True)
    sub.add_parser("state")
    sub.add_parser("stop")
    wav = sub.add_parser("wav", help="播放 PCM16 WAV 文件")
    wav.add_argument("file", type=Path)
    raw = sub.add_parser("raw", help="读取原始 PCM16LE，- 表示 stdin")
    raw.add_argument("file")
    raw.add_argument("--rate", type=int, required=True)
    raw.add_argument("--channels", type=int, choices=(1, 2), default=1)
    tts = sub.add_parser("tts", help="转发 GPT-SoVITS api_v2 的 raw 流")
    tts.add_argument("request", type=Path, help="GPT-SoVITS /tts 请求 JSON")
    tts.add_argument("--url", default="http://127.0.0.1:9880/tts")
    tts.add_argument("--rate", type=int, required=True, help="当前模型的实际输出采样率")
    args = p.parse_args()
    metadata = (("authorization", "Bearer " + os.environ["ROBOTD_API_TOKEN"]),) \
        if os.environ.get("ROBOTD_API_TOKEN") else ()
    with grpc.insecure_channel(args.target, options=[("grpc.enable_http_proxy", 0)]) as channel:
        stub = pg.RobotControlStub(channel)
        if args.command == "state":
            result = stub.GetAudioState(pb.Empty(), timeout=3, metadata=metadata)
        elif args.command == "stop":
            result = stub.StopAudio(pb.Empty(), timeout=3, metadata=metadata)
        elif args.command == "wav":
            with wave.open(str(args.file), "rb") as source:
                if source.getsampwidth() != 2 or source.getcomptype() != "NONE":
                    p.error("WAV 必须是未压缩 PCM16")
                def packets():
                    yield pb.AudioPacket(header=pb.AudioHeader(sample_rate_hz=source.getframerate(),
                                                               channels=source.getnchannels()))
                    while data := source.readframes(1024):
                        yield pb.AudioPacket(pcm=data)
                result = stub.PlayAudio(packets(), metadata=metadata)
        elif args.command == "tts":
            payload = json.loads(args.request.read_text(encoding="utf-8"))
            result = stub.PlayAudio(tts_packets(args.url, payload, args.rate), metadata=metadata)
        else:
            if args.file == "-":
                result = stub.PlayAudio(pcm_packets(sys.stdin.buffer, args.rate, args.channels), metadata=metadata)
            else:
                with open(args.file, "rb") as source:
                    result = stub.PlayAudio(pcm_packets(source, args.rate, args.channels), metadata=metadata)
        show(result)


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
    except grpc.RpcError as e:
        raise SystemExit(f"{e.code().name}: {e.details()}") from None
