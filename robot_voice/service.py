"""车载中文语音对话。机器人动作与语音对话独立。"""
import asyncio
import itertools
import logging
import os
import re
import time
from pathlib import Path

from aiohttp import ClientSession, ClientTimeout, WSMsgType, web
import numpy as np
import sherpa_onnx as sherpa
from paddle_voice import PaddleVoice

log = logging.getLogger("robot-voice")


def load_models(models):
    asr = models / "paraformer"
    model = sherpa.OnlineRecognizer.from_paraformer(
        tokens=str(asr / "tokens.txt"), encoder=str(asr / "encoder.int8.onnx"),
        decoder=str(asr / "decoder.int8.onnx"), num_threads=2)
    tts = models / "kokoro-multi-lang-v1_1"
    kokoro = sherpa.OfflineTtsKokoroModelConfig(
        model=str(tts / "model.onnx"), voices=str(tts / "voices.bin"),
        tokens=str(tts / "tokens.txt"), data_dir=str(tts / "espeak-ng-data"),
        lexicon=f"{tts}/lexicon-us-en.txt,{tts}/lexicon-zh.txt")
    voice = sherpa.OfflineTts(sherpa.OfflineTtsConfig(
        model=sherpa.OfflineTtsModelConfig(kokoro=kokoro, num_threads=3),
        rule_fsts=",".join(str(tts / f) for f in ("phone-zh.fst", "date-zh.fst", "number-zh.fst"))))
    return model, voice


def recognize(model, stream, samples, final=False):
    stream.accept_waveform(16000, samples)
    if final:
        stream.input_finished()
    while model.is_ready(stream):
        model.decode_stream(stream)
    return model.get_result(stream).strip()


def create_app(models):
    model, voice = load_models(models)
    mandarin = PaddleVoice(models)
    turns = itertools.count(1)
    lock = asyncio.Lock()

    async def converse(request):
        ws = web.WebSocketResponse(max_msg_size=65536, heartbeat=15)
        await ws.prepare(request)
        if lock.locked():
            await ws.send_json({"error": "已有语音对话正在进行"})
            await ws.close()
            return ws
        async with lock:
            turn = next(turns)
            stage = "识别"
            started = time.monotonic()
            try:
                hello = await ws.receive_json(timeout=5)
                stream = model.create_stream()
                await ws.send_str("ready")
                count = 0
                partial = ""
                while True:
                    msg = await ws.receive(timeout=15)
                    if msg.type == WSMsgType.TEXT and msg.data == "end":
                        break
                    if msg.type != WSMsgType.BINARY:
                        return ws
                    count += len(msg.data)
                    if count > 16000 * 2 * 60 or len(msg.data) % 2:
                        raise ValueError("每次最多说 60 秒，输入需为 PCM16")
                    samples = np.frombuffer(msg.data, dtype="<i2").astype(np.float32) / 32768
                    current = await asyncio.to_thread(recognize, model, stream, samples)
                    if current and current != partial:
                        partial = current
                        await ws.send_json({"partial": current})
                # 补齐流式编码器的右侧上下文，保留最后一个音节。
                text = await asyncio.to_thread(recognize, model, stream, np.zeros(12800, np.float32), True)
                log.info("轮次=%s 识别=%s 录音=%.2fs", turn, text, count / 32000)
                if not text:
                    raise ValueError("没有识别到语音")
                await ws.send_json({"transcript": text})
                stage = "DeepSeek"
                key = os.getenv("DEEPSEEK_API_KEY")
                if not key:
                    raise ValueError("尚未配置 DeepSeek 密钥")
                async with ClientSession(timeout=ClientTimeout(total=75)) as client:
                    payload = {"model": os.getenv("DEEPSEEK_MODEL", "deepseek-flash"),
                               "messages": [{"role": "system", "content": "用简短自然的中文口语回答，不使用 Markdown。"},
                                            {"role": "user", "content": text}],
                               "thinking": {"type": "disabled"}, "max_tokens": 256}
                    async with client.post("https://api.deepseek.com/chat/completions", json=payload,
                                           headers={"Authorization": "Bearer " + key}) as response:
                        response.raise_for_status()
                        answer = (await response.json())["choices"][0]["message"]["content"].strip()
                    if not answer:
                        raise ValueError("模型返回了空回答")
                    log.info("轮次=%s 回答=%s", turn, answer)
                    await ws.send_json({"answer": answer})
                    stage = "合成播放"
                    async with client.ws_connect("http://127.0.0.1:8080/talk") as speaker:
                        await speaker.send_json({"token": hello["token"], "rate": voice.sample_rate})
                        if await speaker.receive_str(timeout=10) != "ready":
                            raise RuntimeError("音响未就绪")
                        for sentence in re.split(r"(?<=[。！？!?])", answer):
                            if not sentence.strip():
                                continue
                            samples = await asyncio.to_thread(mandarin.generate, sentence)
                            engine = "PaddleSpeech"
                            if samples is None:
                                audio = await asyncio.to_thread(voice.generate, sentence, sid=3)
                                samples = audio.samples
                                engine = "Kokoro"
                            log.info("轮次=%s 合成=%s 音频=%.2fs", turn, engine, len(samples) / voice.sample_rate)
                            pcm = (np.clip(samples, -1, 1) * 32767).astype("<i2").tobytes()
                            for offset in range(0, len(pcm), 4096):
                                await speaker.send_bytes(pcm[offset:offset + 4096])
                        await speaker.send_str("end")
                        result = await speaker.receive_str(timeout=90)
                        if result != "completed":
                            raise RuntimeError("播放失败：" + result)
                log.info("轮次=%s 播放完成 总耗时=%.2fs", turn, time.monotonic() - started)
                await ws.send_json({"done": True})
            except Exception as error:
                log.warning("轮次=%s 阶段=%s 错误=%s", turn, stage, error)
                if not ws.closed:
                    await ws.send_json({"error": f"{stage}：{error}"})
            finally:
                await ws.close()
        return ws

    app = web.Application()
    app.router.add_get("/voice", converse)
    return app


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO, format="%(levelname)s %(message)s")
    web.run_app(create_app(Path(os.getenv("VOICE_MODELS", "models"))), host="127.0.0.1",
                port=8091, access_log=None, handler_cancellation=True)
