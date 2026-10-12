# robotd

robotd 是上层动作程序、调试工具和遥控程序访问底层设备的统一入口。Python 3.11+，部署在树莓派；Windows 可运行模拟服务和客户端。

网页遥控通过独立进程 [robot-remote](../robot_remote/README.md) 接入；它提供 HTTP API 和 WebSocket，不随 robotd 自动启动。

```text
调试 / 动作程序 / 遥控程序
           │ robot.v1.RobotControl (gRPC)
           ▼
     robotd ── SQLite：配置、校准参数、动作定义
       ├── USB CDC → H7 → CAN1：GM6020 yaw、DM4310 pitch
       │                 CAN2/CAN3：RANGER AIR
       ├── UART2 + RTS → 左臂 4 个总线舵机
       ├── UART3 + RTS → 右臂 4 个总线舵机
       └── 本地 AF_UNIX gRPC → eyes 渲染进程
```

## 安装与运行

在仓库根目录：

```sh
python -m venv .venv
# Linux
. .venv/bin/activate
# Windows PowerShell
# .\.venv\Scripts\Activate.ps1
python -m pip install -e .

# 不需要设备即可开发上层控制。
robotd --simulate --db robotd.sqlite
```

另开终端：

```sh
robotctl state
robotctl send robotd/examples/neck-eyes.json --seconds 2
robotctl send robotd/examples/chassis.json --seconds 2
robotctl put-action robotd/examples/greeting.json
robotctl run greeting
robotctl stop --latch
robotctl --clear-latch send robotd/examples/neck-eyes.json
```

未安装时也可用 `python -m robotd.server` / `python -m robotd.client`。退出客户端会释放控制权并停止；`send` 持续续期底盘目标，手臂与颈部位置只发送一次。JSON 浮点数为关节弧度，底盘为 m/s、rad/s 和 rad。

首次接设备前生成并编辑配置，再导入新数据库：

```sh
robotctl default-config robotd/generated-config.json
# 编辑 H7 稳定设备路径、舵机 ID、RTS 极性、方向、中心和结构限位。
robotd --db robotd.sqlite --init robotd/generated-config.json
```

`--init` 只接受新数据库；已有数据库拒绝再次导入，防止启动脚本覆盖权威配置。默认设备启用，示例 H7 路径需替换。任一设备连接失败仍提供配置/状态 RPC，`fault` 给出原因。修改配置或将当前配置重新提交后会再次连接；成功后使用 `clear_latch=true` 取得新控制权。

默认监听 `127.0.0.1:50051`；网络调试可指定 `--listen 0.0.0.0:50051`。服务端和客户端通过环境变量 `ROBOTD_API_TOKEN` 设置同一访问凭据；外部客户端在 metadata 中提供 `authorization: Bearer <token>`。当前传输为明文 gRPC，可在可信内网或 SSH 隧道使用。

## 配置与权威性

SQLite 是唯一持久配置源。配置整体读取、校验和替换，所有字段有 `schema_version`；拼错字段、非法数值、重复总线地址和超出换算范围的限位会被拒绝。`metadata` 是上层共享的信息对象，可存机器人名称、尺寸、坐标标定、能力说明等 JSON 数据。

```sh
robotctl config > current-config.json
# 保留最外层 revision，编辑 config 内的字段。
robotctl update-config current-config.json
```

`UpdateConfig.expected_revision` 必须等于当前版本；冲突返回 `ABORTED`，重新读取再修改。无效/冲突配置不会中断运动。有效更新先停止全部运动、取消动作并撤销租约，再事务提交配置，重新连接并应用设备设置；不会恢复旧目标。已有动作也按候选限位检查，需先修正或删除不再适用的动作。

`GetState.config_revision` 是数据库版本，`applied_config_revision` 是设备已应用版本。应用失败仍保留已提交配置，后者为 0，运动请求被拒绝；可继续读取并修正配置。配置更新 ACK 表示保存完成，设备应用结果查看 `GetState`。

H7 每次连接按数据库参数设置两套位置环；眼屏每次连接应用完整 `eyes.settings`，运行时检查眼屏 UI/重启造成的漂移并覆盖。robotd 不调用眼屏 `SaveSettings`，上层也应统一通过 robotd 修改配置。眼屏姿态是临时目标，不会写入数据库；下次启动恢复数据库指定模式。眼屏素材路径、贴图文件和 H7 引脚/驱动 ID 属于设备部署内容。

每个数据库只允许一个 robotd 进程。树莓派串口以 exclusive 模式打开，禁止其他程序同时占用。

## 设备配置

### H7 与颈部

复用 [Bridge](../h7/tools/dm_can.py) 与 [电机接口](../h7/docs/motors.md)，固件要求 v1.1.0+。当前固件绑定 CAN1/1 Mbps：GM6020 ID1，DM4310 ID0x01/master0x11。H7 保持 1 ms 实时位置闭环，robotd 发位置目标、停止、调参和读状态。

数据库 `neck.yaw/pitch` 包含 `min_rad`、`max_rad`、`direction`、`offset_rad` 及完整 `tuning`。换算为 `H7目标 = direction × 关节角 + offset_rad`；状态额外返回 `joint_position_rad`。默认结构限位 ±0.5 rad，仅作为初始调试配置，需按实际机构修改。

```sh
robotctl zero-neck --mask 3
```

该命令先停止运动，再将 H7 当前反馈设为运行时参考；1=yaw、2=pitch、3=两轴。H7 复位/反馈丢失后必须重新建立机械参考；数据库中的方向、偏置和限位继续保留。H7 现有协议返回相对位置，`ZeroNeck` 不创建可跨掉电恢复的绝对机械零点。

### RANGER AIR

协议以用户提供的 [agv.md](../agv.md) 为准，[官方文档](https://agilexsupport.yuque.com/staff-hso6mo/rg519a/wcm8ggdfnogqogko?singleDoc=)。默认 CAN2/500 kbps，也可改为 CAN3；使用标准经典 CAN 帧，大端字节序。

| 帧 | robotd 行为 |
| --- | --- |
| 0x421 | 连接/取得控制权时请求 CAN 控制模式 |
| 0x141 / 0x291 | 切换运动模式并等待新反馈确认完成 |
| 0x111 | 以目标 20 ms 周期发送速度/转角，保留字节置零 |
| 0x211 / 0x221 | 系统状态、故障、电压和运动反馈 |
| 0x271 / 0x281 | 四轮转角/轮速 |
| 0x251..258 / 0x261..268 | 电机高速/低速反馈 |
| 0x311 / 0x312 | 前后轮里程 |
| 0x241 / 0x361 / 0x362 | 原厂遥控器、BMS、电池报警 |

`ChassisTarget` 明确区分 `ACKERMANN`、`CRAB`、`SPIN`、`PARK`，不把阿克曼转角当作 Twist 角速度。模式切换中、系统故障、反馈过期或原厂遥控器接管时只发送零速度，状态 `devices.chassis.ready=false`。原厂遥控器仍具有最高优先级。

官方速度限值 ±1.5 m/s，转角超过 20° 时 ±0.525 m/s；自旋 ±3.259 rad/s，阿克曼转角 ±0.698 rad，斜移 ±1.571 rad。软件同时执行数据库设置的更小限值，默认速度 0.3 m/s、自旋 0.5 rad/s。原文阿克曼反馈转角的正负说明与指令说明不同，因此返回 `steering_raw_rad`，保留原始反馈符号。

### 双臂 UART 与 RTS

默认 `/dev/ttyAMA2`、`/dev/ttyAMA3`、115200 baud、8N1，每路 ID 0/1/2/3。树莓派 UART 实例名称可能随型号和设备树变化，按实际设备路径修改 `port`，并开启相应 UART 与 RTS 引脚，关闭 RTS/CTS 自动流控。

`rts_tx` 是 pyserial 的 RTS **断言值**（true/false），它与实际引脚高低的对应关系由驱动决定，按现有转接电路配置。发送时切到该值，等待 `flush/tcdrain` 完成，再在 `finally` 中设为相反值释放总线；失败也释放。`direction_mode=manual` 使用这一流程；若 UART 驱动支持 `TIOCSRS485`，可改为 `kernel`，由驱动控制 RTS，设置失败明确报错。

每个关节以 `center_pwm`、`pwm_per_rad`、`direction` 转换到舵机 500..2500 位置；270° 舵机默认 `pwm_per_rad=2000/(270°)`。关节限位必须与实际机构一致。两个串口上的 ID 可以相同，同一串口内不能重复。直连使用逐地址 `#idP位置T时间!`，不依赖专用控制器的批处理命令。

停止使用 `PDST` 保持当前位置，避免机械臂立即卸力下落。位置命令通常无 ACK；后台逐关节发送 `PRAD` 并读取 `#idP位置!`，过滤本机回显，状态带反馈时间或超时原因。后台读取采用分阶段非阻塞处理，避免等待舵机响应占用整个底盘控制周期。舵机需事先处于位置伺服模式。

协议来源为用户提供的《高压总线舵机使用手册》和《协议说明与产品使用说明书(Servo.C)》。旧版/新版的成功回复不同，运动发送结果表示写入完成，实际关节位置通过反馈判断。

### 眼屏

树莓派上运行 `eyes` 渲染进程并指定 `--socket /tmp/oculus-eyes.sock`；robotd 的 `eyes.target` 为 `unix:/tmp/oculus-eyes.sock`。由 gRPC protobuf 直接传 `Pose` 与 `BlinkRequest`，无需转换 JSON。配置保留全部现有眼屏字段，包括果冻、图层、模拟器、眨眼、外观、窗口与面捕。

双眼 `openness` 正常睁眼请填 1；人物自身左眼位于屏幕右侧。临时 `SetPose` 自动进入 External 模式，配置更新/重启恢复持久模式。Windows 上可用模拟 robotd 开发上层；实际 AF_UNIX 眼屏集成面向 Linux 部署。

## gRPC 使用约定

协议：[robot.proto](proto/robot.proto)，眼屏导入来自 `eyes/proto/eyes.proto`。发布包附带生成代码和 `pb/eyes.proto` 快照；更新眼屏协议后运行 `python robotd/generate.py` 重新生成。

| 方法 | 用途 |
| --- | --- |
| GetConfig / UpdateConfig | 权威配置查询与整体替换 |
| GetState / WatchState | 当前运行状态与订阅 |
| AcquireControl / RenewControl / ReleaseControl | 独占控制权、续期和释放 |
| SetTargets / StreamTargets | 单次/客户端流目标，支持多个设备组合 |
| Stop / ZeroNeck | 全部停止、停止锁定及颈部运行时归零 |
| ListActions / PutAction / DeleteAction | 版本化动作存储 |
| RunAction / CancelAction | 时间点动作执行与取消 |

取得控制权后带返回的 token 下发目标，租约 200..5000 ms，必须显式续期。控制权到期/释放会停止底盘、颈部和双臂并取消动作。`Stop` 不需要租约，可抢占排队请求；它撤销旧 token。`latch=true` 后，新的 Acquire 必须显式 `clear_latch=true`。停止状态表示中枢已清除运动目标并请求设备停止，实际执行结果查看故障与设备反馈。

底盘目标另有 100..1000 ms 的 `ttl_ms`；租约心跳不能延长旧速度目标。到期会单独停止底盘并驻车。颈部、机械臂位置保持至新目标或租约到期。所有目标先整体验证，再访问设备；设备 I/O 失败会停止整机、关闭连接并进入故障状态。ACK/revision 表示目标已接受，不表示到位。

控制线程串行拥有 H7、两个 UART 与数据库。普通请求队列最多 64 条，满时拒绝；状态/控制长连接最多 8 条，保留 unary RPC 工作线程。停止/取消请求具有队列优先级。请求超时不会自动重发，客户端应查询状态。

## 动作开发

动作是 `name + duration_ms + steps`；每个 `step.at_ms` 指定发送目标的相对时间。时间点严格递增，最长 10 分钟/1000 点。H7 负责颈部闭环，舵机自行执行 `duration_ms` 轨迹，robotd 不额外实现 IK。可用上层规划器生成关节时间点。

动作保存和更新检查自身 revision；新动作 expected_revision=0，更新使用 ListActions 返回值。删除也保留版本号，ListActions 中标为 `deleted=true`，重建同名动作使用该版本，避免旧客户端覆盖新动作。运行中拒绝修改/删除同名动作和直接 SetTargets，需先取消。开始动作会停止已有目标；正常完成、取消、失去租约都停止整机。调度时间点迟到超过 250 ms 会中止，避免跳过时间点后突然执行旧动作。

底盘动作仍受每段目标 TTL 限制，长时间运动需增加时间点，或使用持续目标流。上层参考 [remote.py](examples/remote.py) 和客户端 `Session`，它负责控制权续期与退出释放。

## 部署和检查

`deploy/robotd.service` 给出 systemd 示例，项目装到 `/opt/robot` 的虚拟环境，`robot` 用户加入串口权限组。先用实际配置创建 `/var/lib/robotd/robotd.sqlite` 并调整权限，再启用服务；眼屏服务需在 robotd 前启动。自动重启不会恢复旧动作或速度目标。

```sh
python -m unittest discover -s robotd/tests -v
python -m build --wheel --no-isolation
```

检查覆盖文档中的 CAN 向量、真实 H7 Bridge CDC 拆包、模式反馈门控、RTS drain/释放/回显、SQLite 重开/版本冲突/实例锁、整条目标校验、租约/速度 TTL、动作取消/延迟停止，以及实际本机 gRPC 配置/指令流/眼屏 RPC 往返。设备由测试传输模拟；实机接线、RTS 极性、机构标定和负载闭环需在树莓派上确认。
