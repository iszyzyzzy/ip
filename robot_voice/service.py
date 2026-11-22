"""车载中文语音对话。机器人动作与语音对话独立。"""
import asyncio
import io
import itertools
import json
import logging
import os
import time
import wave
from pathlib import Path

from aiohttp import ClientSession, ClientTimeout, WSMsgType, web
from piper import PiperVoice
from vosk import KaldiRecognizer, Model

log = logging.getLogger("robot-voice")


def create_app(models):
    model = Model(str(models / "vosk-model-small-cn-0.22"))
    voice = PiperVoice.load(str(models / "voice.onnx"))
    turns = itertools.count(1)
    lock = asyncio.Lock()

    def synthesize(text):
        buffer = io.BytesIO()
        with wave.open(buffer, "wb") as wav:
            voice.synthesize_wav(text, wav)
        buffer.seek(0)
        return buffer

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
                recognizer = await asyncio.to_thread(KaldiRecognizer, model, 16000)
                await ws.send_str("ready")
                parts = []
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
                    if await asyncio.to_thread(recognizer.AcceptWaveform, msg.data):
                        parts.append(json.loads(recognizer.Result())["text"])
                    else:
                        current = json.loads(recognizer.PartialResult())["partial"]
                        if current and current != partial:
                            partial = current
                            await ws.send_json({"partial": "".join(parts) + current})
                parts.append(json.loads(recognizer.FinalResult())["text"])
                text = "".join(parts).replace(" ", "").strip()
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
                    audio = await asyncio.to_thread(synthesize, answer)
                    with wave.open(audio, "rb") as wav:
                        async with client.ws_connect("http://127.0.0.1:8080/talk") as speaker:
                            await speaker.send_json({"token": hello["token"], "rate": wav.getframerate()})
                            if await speaker.receive_str(timeout=10) != "ready":
                                raise RuntimeError("音响未就绪")
                            while pcm := wav.readframes(1024):
                                await speaker.send_bytes(pcm)
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
