"""车端远程语音合成，所有后端输出单声道 PCM16。"""
import asyncio
import base64
from email.utils import formatdate
import hashlib
import hmac
import json
import os
from urllib.parse import quote, urlencode


def required(name):
    value = os.getenv(name)
    if not value:
        raise ValueError(f"车端未配置 {name}")
    return value


async def deepseek(client, text):
    headers = {"Authorization": "Bearer " + required("DEEPSEEK_API_KEY")}
    body = {"text": text, "format": "pcm", "voice_id": os.getenv("DEEPSEEK_TTS_VOICE", "stella")}
    async with client.post("https://api.deepseek.com/v1/tools/tts", json=body, headers=headers) as response:
        response.raise_for_status()
        tail = b""
        async for chunk in response.content.iter_chunked(4096):
            data = tail + chunk
            end = len(data) & ~1
            if end:
                yield data[:end]
            tail = data[end:]
        if tail:
            raise ValueError("DeepSeek 返回了不完整的 PCM 采样")


async def xfyun(client, text):
    host = "tts-api.xfyun.cn"
    date = formatdate(usegmt=True)
    signed = f"host: {host}\ndate: {date}\nGET /v2/tts HTTP/1.1"
    signature = base64.b64encode(hmac.digest(required("XFYUN_API_SECRET").encode(), signed.encode(), hashlib.sha256)).decode()
    auth = (f'api_key="{required("XFYUN_API_KEY")}", algorithm="hmac-sha256", '
            f'headers="host date request-line", signature="{signature}"')
    params = urlencode({"authorization": base64.b64encode(auth.encode()).decode(), "date": date, "host": host})
    body = {"common": {"app_id": required("XFYUN_APP_ID")},
            "business": {"aue": "raw", "auf": "audio/L16;rate=16000", "tte": "UTF8",
                         "vcn": os.getenv("XFYUN_VOICE", "xiaoyan")},
            "data": {"status": 2, "text": base64.b64encode(text.encode()).decode()}}
    # 握手异常可能带签名 URL，避免写入客户端提示和日志。
    try:
        ws = await client.ws_connect(f"wss://{host}/v2/tts?{params}")
    except Exception as err:
        raise RuntimeError(f"讯飞连接失败（{getattr(err, 'status', type(err).__name__)}）") from None
    async with ws:
        await ws.send_json(body)
        while True:
            event = await asyncio.wait_for(ws.receive_json(), 30)
            if event["code"] != 0:
                raise ValueError(f"讯飞 {event['code']}：{event.get('message', '')}")
            data = event["data"]
            if data.get("audio"):
                yield base64.b64decode(data["audio"])
            if data["status"] == 2:
                return


async def doubao(client, text):
    voice = os.getenv("DOUBAO_VOICE") or "multi_female_tuanzi_v3_uranus_bigtts"
    url = f"https://voice.cn-beijing.volces.com/api/v3/text-to-speech/{quote(voice, safe='')}/streaming"
    body = {"text": text, "model_id": "doubao-seed-tts-3.0",
            "output_format": {"format": "pcm", "sample_rate": 24000}}
    async with client.post(url, json=body, headers={"X-Api-Key": required("DOUBAO_API_KEY")},
                           read_bufsize=1024 * 1024) as response:
        async for line in response.content:
            if not line.strip():
                continue
            if response.status >= 400 and not line.lstrip().startswith(b"{"):
                response.raise_for_status()
            event = json.loads(line)
            if event.get("error"):
                error = event["error"]
                raise ValueError(f"豆包 {error.get('code', response.status)}：{error.get('message', '')}")
            response.raise_for_status()
            if event["event"] == "audio":
                pcm = base64.b64decode(event["audio"])
                for offset in range(0, len(pcm), 4096):
                    yield pcm[offset:offset + 4096]
            elif event["event"] == "final" and event.get("reason") == "TTS_STREAM_FINAL_REASON_CLOSED":
                return
        response.raise_for_status()
        raise RuntimeError("豆包合成流提前结束")
