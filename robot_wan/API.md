# 公网遥控 API v1

电脑 APP 与车端使用 WebRTC。公网信令负责认证、车辆列表和会话协商。ICE 使用候选地址选择直连或 coturn 中继。连接后通过 WebRTC DataChannel 遥控，通过标准 WebRTC RTP 轨道传输音视频。应用层报文在两种路径上相同。

## 公网信令

在 `GET /v1/signal` 升级 WebSocket，请求头带 `Authorization: Bearer <token>`。公网入口采用 WSS。操作员与车辆使用各自固定的凭据，首版一个信令实例管理一辆车和一个活动远程会话。未授权的握手返回 HTTP 401。`GET /healthz` 返回 `{"ok":true}`。

每条消息为 UTF-8 JSON 对象。请求可带 `id`，直接响应回显该字段。与会话相关的转发和关闭通知携带 `sid`。信令任一端 WebSocket 断开便关闭会话，客户端重连后重新打开并协商。

| APP 发送 | 字段 | 接收 |
| --- | --- | --- |
| `robots.list` | 可选 `id` | `robots.list` 的 `robots[]`，元素含 `robot_id`、`online`、`busy`、`capabilities` |
| `session.open` | `robot_id`、`version: 1` | `session.opened` 含 `sid`、`robot_id`、`capabilities` 和 `ice_servers` |
| `offer` | `sid`、`sdp` | 车端发回 `answer`，含 `sid`、`sdp` |
| `ice` | `sid`、`candidate` | 对端可按同格式发消息。车端的本地候选直接放入 answer SDP |
| `session.close` | `sid` | `session.closed` 含 `sid`、`reason` |

`candidate` 为 `{"candidate":"candidate:…","sdpMid":"0","sdpMLineIndex":0}`。候选结束时填 JSON `null`。可直接把收集完毕的候选放到 offer SDP。使用增量候选的 APP 先发 offer。首版每个会话协商一次，切网后需要重新协商时关闭旧会话并打开新会话。

`ice_servers` 为 WebRTC ICE server 对象数组。STUN 项含 `urls`，TURN 项额外含 `username` 和 `credential`。每个实例提供一个 TURN URL。测试网络可不配置 ICE 服务，此时先验证主机候选直连。车辆使用独立凭据发送 `{"type":"robot.hello","robot_id":"car-01","version":1,"capabilities":{"audio":true,"video":true}}`，等待 `robot.ready`。

信令错误形如 `{"type":"error","id":"s1","error":{"code":"BUSY","message":"车辆已有远程会话"}}`。常见错误有 `ROBOT_OFFLINE`、`BUSY`、`INVALID_ARGUMENT`、`NOT_FOUND`。

## WebRTC 协商

收到 `session.opened` 后，用其中的 ICE 服务创建 PeerConnection，再创建以下 DataChannel 和媒体轨，然后生成 offer。DataChannel 使用 in-band 协商。首次打开后等待 `state.v1` 发回车端时间标记。

| 通道名 | 参数 | 用途 |
| --- | --- | --- |
| `rpc.v1` | `ordered=true`，可靠 | 离散操作、续租和喊话 |
| `drive.v1` | `ordered=false, maxRetransmits=0` | 只有 APP 发实时底盘目标 |
| `state.v1` | `ordered=false, maxRetransmits=0` | 只有车端发状态快照 |
| `stop.v1` | `ordered=true`，可靠 | 独立急停请求和应答 |

APP 的麦克风音轨使用 **sendonly**，SDP 须包含 Opus。开始喊话前保持静音。相机视频使用 **recvonly**，SDP 须包含 VP8。若无需某种媒体，可在 offer 中省略该媒体段。`capabilities` 表示车端配置能力，运行时状态见 `state.v1`。

## 命令与状态

`rpc.v1` 和 `stop.v1` 的请求结构为 `{"id":1,"method":"…","params":{}}`。答复为 `{"type":"reply","id":1,"result":{…}}` 或 `{"type":"reply","id":1,"error":{"code":"…","message":"…"}}`。接口字段采用 protobuf JSON 的 snake_case 名称，`uint64`（如配置 revision）在响应中为字符串。急停通道上仅使用 `{"id":99,"method":"stop","params":{"latch":true}}`，服务端始终锁定停止。

| `method` | 主要 `params` | 结果 |
| --- | --- | --- |
| `get_state` / `get_config` / `list_actions` | `{}` | `RobotState` / `ConfigSnapshot` / `ActionList` |
| `acquire` | `tick`、可选 `clear_latch` | `epoch`、`ttl_ms: 1000` |
| `renew` | `epoch`、`tick` | 同接管结果，建议每 300 ms 显式发送 |
| `release` | `epoch` | 释放并停止 |
| `targets` | `epoch`、`tick`、`targets` | 颈部、手臂、单舵机、眼屏或眨眼目标 |
| `run_action` / `cancel_action` | `epoch`、`tick`，执行时另带 `name` | 接受执行或取消，完成情况看状态 |
| `zero_neck` | `epoch`、`tick`、`mask` | 设置颈部运行时零位 |
| `update_config` / `update_eyes` | 对应的 `ConfigUpdate` / `EyesUpdate` | 变更后的配置和 revision |
| `put_action` / `capture_preset` / `delete_action` | 对应的 `ActionUpdate` / `PresetCapture` / `ActionDelete` | 动作记录或确认 |
| `get_gamepad` / `set_gamepad` | 查询为空，设置带 `chord`、`action` | 树莓派本机蓝牙手柄状态和绑定 |
| `talk.begin` / `talk.end` | `{}` | 喊话请求状态，实际结果见状态快照 |

结构体细节见 [robot.proto](../robotd/proto/robot.proto) 和 [现有配置命令](../robot_remote/README.md)。APP 无需处理 robotd 的租约 token。整车配置更新后需要重新接管。离散请求超时后应查询执行状态。

`state.v1` 每约 100 ms 推送一条快照。字段为 `seq`、`tick`、`epoch`、`state`（或后端不可用的 `error`）、`video`、`talk`、`dropped_drive`。APP 只使用最新 `seq`。最近 500 ms 内的车端 `tick` 用于接管、续租和离散运动。断开会话会释放租约，租约自身也会在停止续期后到期。

取得 `epoch` 后，APP 可按 20 Hz 发 `drive.v1`。下例单位为 m/s、rad 和 rad/s。收到控制权后从正整数 `seq=1` 开始递增，松手立即发零速。车端拒绝旧 epoch、乱序或超过 200 ms 的 tick，并自行计算最长 200 ms 的底盘目标 TTL。

```json
{"epoch":"<acquire 返回>","seq":1,"tick":483400,"chassis":{"mode":"ACKERMANN","speed_m_s":0.1,"steering_rad":0,"yaw_rate_rad_s":0}}
```

喊话开始后开放麦克风，松手发送 `talk.end` 并静音。车端把 Opus 解码为 48 kHz 单声道 PCM16，交给 robotd。现有播放器占用时返回 `AUDIO_BUSY`，已有流不会被抢占。正常松手保留短尾音，急停或会话关闭取消喊话。状态中的 `talk.phase` 可见 `idle`、`accepted`、`completed`、`failed`、`offline`，失败带 `code` 和 `message`。相机状态为 `inactive`、`starting`、`live` 或 `offline`。画面是否冻结由 APP 实际收帧时间判定。

## 启动与切换

项目根目录执行 `cargo build --release --locked --manifest-path robot_wan/Cargo.toml`，两份二进制位于 `robot_wan/target/release/`。Debian 系 Linux 构建需要 `pkg-config`、`libasound2-dev` 和 C 编译工具链。systemd 示例从 `/opt/robot-wan/bin/` 启动。公网可以用反向代理终止 TLS，转发 `/v1/signal` 的 WebSocket 到本机 8765。用 [coturn 示例](deploy/turnserver.conf.example) 配置 TURN 并开放输入端口及 UDP 中继范围。信令进程从 `WAN_ROBOT_TOKEN`、`WAN_OPERATOR_TOKEN`、`WAN_TURN_SECRET` 读取凭据，最后一个与 coturn 的 `static-auth-secret` 相同。生产示例命令：

```sh
robot-wan-signal --robot-id car-01 \
  --stun stun:turn.example.com:3478 \
  --turn 'turn:turn.example.com:3478?transport=udp'
```

Rust 车端的 TURN 连接使用 UDP，部署网络须允许车端访问 TURN 的 UDP 端口。车端配置相同的 `WAN_ROBOT_TOKEN` 和所需的 `ROBOTD_API_TOKEN`。视频需要带 libvpx 的 FFmpeg，车端账号需要摄像头读取权限。配置好车载相机时使用：

```sh
robot-link --robot-id car-01 --signal wss://robot.example.com/v1/signal \
  --camera /dev/v4l/by-id/你的相机-video-index0
```

车端固定以 640×480、15 fps 采集。从模拟车验证视频时可用 `--camera-format lavfi --camera testsrc=size=640x480:rate=15`。可省略 `--camera` 验证纯控制会话。车端设置中须启用 robotd 的 `audio.enabled` 才能播放喊话。

上线时安装并修改 [车端 systemd 模板](deploy/robot-link.service) 的信令地址、相机参数和凭据文件。随后执行 `systemctl disable --now robot-remote.service` 并启用 `robot-link.service`。两个服务配置为互斥，避免同时读取蓝牙手柄。旧网页服务文件和资产留作回退，蓝牙手柄绑定沿用运行账号的 `~/.config/robot-remote/gamepad.json`。公网信令另有 [systemd 模板](deploy/robot-wan-signal.service)。

本地先构建 `robotd-rs` 和 `robot_wan` 的 debug 二进制，再用 `uv run --extra wan-test python -m unittest discover -s robot_wan/tests -v` 启动测试对端。测试使用 FFmpeg 合成相机和模拟麦克风。跨网 NAT/TURN、实车 UVC 与 ALSA 端到端延迟应在各自的实际环境中验证。
