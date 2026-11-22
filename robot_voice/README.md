# 车载中文语音对话

从 HTTPS 遥控器点「语音对话」，说完点「说完了」。浏览器将 16 kHz 单声道 PCM 发到遥控网关的 `/voice`。网关校验访问凭据后转发到本机 8091。Vosk 增量识别普通话，DeepSeek 接收本轮识别文本，Piper 合成中文回答，通过既有音响通道播放。每次对话独立，音量沿用网页音量条。

实机目录为 `~/robot-voice`，Python 环境安装 `vosk==0.3.45`、`piper-tts==1.8.0`、`aiohttp>=3.12,<4`。将 [Vosk 中文小模型](https://alphacephei.com/vosk/models/vosk-model-small-cn-0.22.zip) 解压到 `models/`，将 [Piper huayan medium](https://huggingface.co/rhasspy/piper-voices/tree/main/zh/zh_CN/huayan/medium) 的 ONNX 和配套 JSON 分别保存为 `models/voice.onnx` 和 `models/voice.onnx.json`。复制本目录的 service.py，安装 systemd 单元并启用。

在树莓派上用 `sudoedit /etc/robot-voice.env` 填写：

```ini
DEEPSEEK_API_KEY=
DEEPSEEK_MODEL=deepseek-flash
```

凭据文件由 root 持有，权限 0600。修改后运行 `sudo systemctl restart robot-voice`。用 `journalctl -u robot-voice -f -o cat` 查看每轮识别文本、模型回答、播放完成或失败阶段。
