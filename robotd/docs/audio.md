# 流式音频播放

服务端为纯 Rust，音频队列和 ALSA 播放线程均在 robotd 进程内。上游 GPT-SoVITS 独立运行，robotd 不加载模型、不保存整段音频、不启动播放器子进程。

## 接线与系统声卡

按提供的图，树莓派输出为：

| 接口 | BCM GPIO | 物理针脚 | 功放信号 |
| --- | --- | --- | --- |
| PCM_CLK | 18 | 12 | I²S BCLK |
| PCM_FS | 19 | 35 | I²S LRCLK/WS |
| PCM_DOUT | 21 | 40 | I²S DIN |

TAS5756M 模块已预配置，直接使用系统已启用的 ALSA 设备。用 `aplay -l` / `aplay -L` 查找声卡；优先指定稳定的声卡名称，例如 `plughw:CARD=你的I2S声卡,DEV=0`。`plughw` 由 ALSA 转换声道、格式和采样率；不要将设备固定为另一个采样率后直接播放不同采样率的 raw PCM。

从 `GetConfig` 导出配置，修改 audio 后通过 `UpdateConfig` 提交：

```json
{
  "enabled": true,
  "device": "plughw:CARD=你的I2S声卡,DEV=0",
  "prebuffer_ms": 100,
  "rebuffer_ms": 80,
  "max_buffer_ms": 160,
  "device_buffer_ms": 40
}
```

audio 默认关闭，避免旧部署升级后错误占用系统默认声卡。Linux 构建使用 `libasound2-dev`，运行只需系统 `libasound.so.2`；不需要 Python。声卡以单个流按需打开，播完或停止后关闭。

## 缓冲与响应时间

收到 100 ms **音频时长**的数据后开始提交声卡，不是固定等候 100 ms：上游一次给足数据就立即开始。每次向声卡提交约 10 ms；ALSA 目标缓冲 40 ms，预留完整设备缓冲抵抗 Linux 调度抖动。默认软件队列最多 160 ms，32 kHz 单声道 PCM16 为 **10 KiB**；48 kHz 双声道为 30 KiB。软件队列与声卡缓冲的配置之和最多 200 ms，声卡协商出的实际缓冲也会检查并报告。

首段不足 100 ms 时，发送端结束输入即可立即播放尾部。播放中断流时插入静音，保留后续数据顺序，积累 80 ms 后继续。上游生成过快时，满队列暂停接收，利用 gRPC/HTTP2 背压控制流量，不丢弃语音或持续扩展队列。

100–200 ms 是此播放链路的低延迟配置目标，TTS 首包生成时间另计。`start_latency_ms` 测量首个 PCM 字节到首次提交播放后端的时间，`device_buffer_ms` 是 ALSA 协商出的缓冲时长；它们不是麦克风实测的端到端出声延迟。

## gRPC 接口

`robot.v1.RobotControl.PlayAudio(stream AudioPacket) -> AudioStatus`：

1. 首包是 `AudioHeader { sample_rate_hz, channels }`。
2. 后续都是 `pcm` 字节。格式固定为 **有符号 PCM16LE**；立体声按 L/R 交错，声道数 1 或 2，每包最多 64 KiB。
3. 网络分包可落在半个采样值或采样帧中间，服务端会拼接；整个流末尾必须是完整帧。
4. 发送端半关闭输入后，服务端排空全部尾部及声卡缓冲，再返回 `completed`。HTTP 分块结束不会作为独立声音片段重新初始化声卡。

采样率支持 8/16/22.05/24/32/44.1/48/88.2/96/192 kHz。raw PCM 没有自带采样率，Header 必须与上游模型输出一致。WAV/AAC/OGG 等容器或编码不能直接作为 PCM 发入。

同时只允许一个音频流；已有流输入或播放中时返回 `RESOURCE_EXHAUSTED`。`GetAudioState` 查询独立状态，`GetState.audio` 和状态订阅也包含音频状态；`StopAudio` 清空队列并取消当前流。客户端断连、取消 RPC、全局 `Stop`、配置更新或 robotd 退出都会取消该流，旧流不会影响随后新建的流。

音频使用服务端访问凭据，不要求运动租约。运动控制权到期、遥控器释放或动作完成只停止运动，语音继续；全局停止锁定后拒绝新的音频流。

状态包括 `buffering/playing/rebuffering/draining/completed/stopping/cancelled/failed`，以及接收帧数、向播放后端提交的帧数、静音帧数、缓冲峰值、恢复缓冲次数和错误。`submitted_frames` 表示已交给后端，正常 `completed` 才表示后端已排空；模拟模式不产生声音。

## 工具与 GPT-SoVITS

```sh
uv run robot-audio --target 机器人IP:50051 wav hello.wav
uv run robot-audio --target 机器人IP:50051 raw speech.pcm --rate 32000 --channels 1
uv run robot-audio --target 机器人IP:50051 state
uv run robot-audio --target 机器人IP:50051 stop

# 编辑请求中的参考音频路径、语言和文本；路径属于 GPT-SoVITS 服务器。
uv run robot-audio --target 机器人IP:50051 tts robotd/examples/tts.json \
  --url http://GPT-SOVITS服务器:9880/tts --rate 32000
```

转发工具请求 `media_type=raw` 和流式模式，用 `read1` 读取已可用的 HTTP 数据，不等待填满大缓冲。`--rate` 必填：不同模型输出可能不同，不能把 32 kHz 当作固定协议。GPT-SoVITS 参数参考其 [api_v2.py](https://github.com/RVC-Boss/GPT-SoVITS/blob/main/api_v2.py)。

服务端的 ALSA 使用方式参考 [alsa-rs PCM 接口](https://docs.rs/alsa/latest/alsa/pcm/index.html)。驱动与功放寄存器不由 robotd 管理。
