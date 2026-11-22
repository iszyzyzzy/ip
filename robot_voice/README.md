# 车载中文语音对话

从 HTTPS 遥控器点「语音对话」，说完点「说完了」。浏览器将 16 kHz 单声道 PCM 发到遥控网关的 `/voice`。网关校验访问凭据后转发到本机 8091。Paraformer 流式识别中英文，DeepSeek 接收本轮识别文本，本地合成回答，通过既有音响通道播放。每次对话独立，音量沿用网页音量条。

实机目录为 `~/robot-voice`，Python 环境安装 `sherpa-onnx==2.0.3`、`numpy` 和 `aiohttp>=3.12,<4`。

- 从 [流式 Paraformer 模型](https://huggingface.co/csukuangfj/sherpa-onnx-streaming-paraformer-bilingual-zh-en) 下载 `encoder.int8.onnx`、`decoder.int8.onnx` 和 `tokens.txt` 到 `models/paraformer/`。
- 将 [Kokoro v1.1 中文模型包](https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_1.tar.bz2) 解压到 `models/`，保留 `kokoro-multi-lang-v1_1/` 目录和其中的词典。Kokoro 音色为普通话女声 zf_001。80 字符以内的回答整段合成播放，较长回答按句合成。

普通话、英语及混合句使用 PaddleSpeech FastSpeech2 Mix 的 174 号说话人与 HiFi-GAN。英文使用 CMU 发音词典，短大写缩写及词典外单词按字母读。遇到模型音素表无法表示的句子才使用 Kokoro。安装 `onnxruntime`、`pypinyin==0.55.0`、`cn2an`、`cmudict`，将官方 [FastSpeech2](https://paddlespeech.cdn.bcebos.com/t2s/chinse_english_mixed/models/fastspeech2_mix_onnx_0.2.0.zip) 和 [HiFi-GAN](https://paddlespeech.cdn.bcebos.com/Parakeet/released_models/hifigan/hifigan_csmsc_onnx_0.2.0.zip) 压缩包直接解压到 `models/`。

复制本目录的 service.py 和 paddle_voice.py，安装 systemd 单元并启用。

在树莓派上用 `sudoedit /etc/robot-voice.env` 填写：

```ini
DEEPSEEK_API_KEY=
DEEPSEEK_MODEL=deepseek-flash
```

凭据文件由 root 持有，权限 0600。修改后运行 `sudo systemctl restart robot-voice`。用 `journalctl -u robot-voice -f -o cat` 查看每轮识别文本、模型回答、播放完成或失败阶段。
