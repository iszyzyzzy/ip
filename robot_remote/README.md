# 网页遥控服务 robot-remote

独立 HTTP/WebSocket 进程，通过 gRPC 连接 robotd。可部署在 robotd 所在树莓派，也可部署在另一台机器。网关只保留临时控制会话；配置、动作、设备状态和控制权仍以 robotd 为准。

```text
网页遥控器 ── HTTP / WebSocket ── robot-remote :8080 ── gRPC ── robotd :50051
```

## 安装和启动

在仓库根目录安装额外的 HTTP 依赖：

```sh
python -m pip install -e ".[remote]"
```

分开启动两个进程：

```sh
# 终端 1；接实机时去掉 --simulate，使用已配置的数据库。
robotd --simulate --listen 127.0.0.1:50051 --db robotd.sqlite

# 终端 2；也可使用 python -m robot_remote.server。
robot-remote --robotd 127.0.0.1:50051 --host 0.0.0.0 --port 8080
```

robotd 不需要安装 `[remote]`、启动网关或开放 HTTP 端口。两者目前在同一 Python 发布包中，启动、退出和部署均独立。如果分机部署，`--robotd` 指向实际 robotd 地址，并让 robotd 监听可达的接口。

已有网页目录可以随网关提供：

```sh
robot-remote --web-root /opt/robot/web-remote --host 0.0.0.0
```

仓库内的网页遥控器见 [web_remote_control](../web_remote_control/README.md)，`npm run build` 后使用 `--web-root web_remote_control/dist`。目录需包含 `index.html`；访问 `/` 即可打开，静态资源和 API 同源。开发时前端也可单独运行：

```sh
robot-remote --allow-origin http://localhost:5173
```

默认允许同源网页及无 Origin 的命令行客户端。其他网页来源必须使用 `--allow-origin` 指定，可重复添加；HTTP 预检和 WebSocket 握手均检查来源。使用 HTTPS 反向代理时，可显式添加网页的 HTTPS 来源，并由代理转发 `/api/`、`/ws`；网页使用 `wss://`。

环境变量：

| 变量 | 用途 |
| --- | --- |
| `ROBOTD_API_TOKEN` | 网关访问 robotd 的 gRPC 凭据，与 robotd 的设置一致 |
| `ROBOT_REMOTE_API_TOKEN` | 网页/HTTP 客户端访问网关的凭据，独立于 robotd 凭据 |

变量为空时对应连接无需凭据。对局域网开放服务时设置网页凭据。HTTP 请求携带 `Authorization: Bearer <网页凭据>`；浏览器 WebSocket 在连接后发送 `auth`。凭据不放 URL。网关默认监听 `127.0.0.1:8080`。

## HTTP API

所有写请求使用 `Content-Type: application/json`。成功返回 `{"result": ...}`，错误返回 `{"error":{"code":"INVALID_ARGUMENT","message":"..."}}`。错误 code 保留 robotd 的 gRPC 状态名称；常见 HTTP 状态为 400 参数错误、401 未认证、403 会话/来源错误、409 版本冲突或状态不允许、429 控制权占用、503 后端不可用、504 后端超时。

| 方法和路径 | 请求 JSON | 对应 WebSocket 命令 |
| --- | --- | --- |
| `GET /healthz` | 无；返回 `{"ok":true,"robotd":"connected"}`，后端失败返回 503/504 | — |
| `GET /api/state` | 无 | `get_state` |
| `GET /api/config` | 无 | `get_config` |
| `PUT /api/config` | `expected_revision`, `config` | `update_config` |
| `GET /api/actions` | 无 | `list_actions` |
| `POST /api/actions` | `expected_revision`, `action` | `put_action` |
| `DELETE /api/actions` | `expected_revision`, `name` | `delete_action` |
| `POST /api/control/acquire` | `owner`, `ttl_ms`, `clear_latch`，均可省略 | `acquire` |
| `POST /api/control/renew` | `session_id` | `renew` |
| `POST /api/control/release` | `session_id` | `release` |
| `POST /api/targets` | `session_id`, `targets` | `targets` |
| `POST /api/stop` | `latch`，默认 false，无需会话 | `stop` |
| `POST /api/neck/zero` | `session_id`, `mask`（1=yaw、2=pitch、3=两轴） | `zero_neck` |
| `POST /api/actions/run` | `session_id`, `name` | `run_action` |
| `POST /api/actions/cancel` | `session_id` | `cancel_action` |

参数沿用 [robot.proto](../robotd/proto/robot.proto) 的 snake_case 字段及 Protobuf JSON 格式；uint64 revision 返回十进制字符串，回传时保留字符串即可。未知字段拒绝。`targets` 支持底盘、颈部、左右臂、眼睛及眨眼，单位与 [robotd](../robotd/README.md) 一致。

`acquire` 默认 `owner="web-remote"`、`ttl_ms=1000`，范围 200..5000 ms。返回：

```json
{"result":{"session_id":"临时会话标识","owner":"web-remote","ttl_ms":1000}}
```

网关保管真实 robotd lease token，客户端使用 `session_id`。HTTP 会话允许客户端在不同 HTTP 连接上继续使用；WebSocket 会话绑定建立它的连接，不能混用。HTTP 连接结束不释放会话，需显式 release 或等待租约到期。

无凭据的本机模拟示例（PowerShell 使用 `curl.exe`）：

```sh
curl http://127.0.0.1:8080/api/state
curl -X POST http://127.0.0.1:8080/api/control/acquire \
  -H 'Content-Type: application/json' -d '{"owner":"http-remote"}'
# 将返回的 session_id 填入下方。
curl -X POST http://127.0.0.1:8080/api/targets \
  -H 'Content-Type: application/json' \
  -d '{"session_id":"...","targets":{"chassis":{"mode":"ACKERMANN","speed_m_s":0.1,"ttl_ms":200}}}'
curl -X POST http://127.0.0.1:8080/api/stop \
  -H 'Content-Type: application/json' -d '{"latch":true}'
```

## WebSocket 协议

连接 `/ws`。服务端首先发出：

```json
{"type":"ready","protocol":"robot-remote.v1","authenticated":false,"state_interval_ms":100}
```

启用凭据时，浏览器在 5 秒内发送认证消息；认证前不推送状态。凭据错误/未认证操作会关闭连接。无需凭据或握手已带正确 Authorization 时，`authenticated=true`，可以直接发送命令。

```json
{"id":1,"method":"auth","params":{"token":"网页访问凭据"}}
{"id":2,"method":"acquire","params":{"owner":"browser","ttl_ms":1000}}
{"id":3,"method":"targets","params":{"targets":{"neck":{"yaw_rad":0.1}}}}
{"id":4,"method":"renew","params":{}}
{"id":5,"method":"release","params":{}}
```

每个命令须携带整数或最长 128 字符的字符串 `id`；响应使用相同 id。WebSocket 控制命令自动使用当前连接取得的会话，通常省略 `session_id`。每条连接同时只能持有一个会话，整个机器人控制权仍然独占。

```json
{"type":"reply","id":3,"result":{"revision":"7"}}
{"type":"reply","id":3,"error":{"code":"INVALID_ARGUMENT","message":"..."}}
{"type":"state","state":{"revision":"7","stopped":false,"owner":"browser"}}
{"type":"backend_error","error":{"code":"UNAVAILABLE","message":"..."}}
```

状态消息无需请求，默认每 100 ms 推送一次，使用 `--state-interval-ms` 调整。所有网页共用一个带超时的 robotd 状态查询；慢连接仅保留最新快照。`state` 实际包含完整 RobotState，示例省略其他字段。指令响应与状态消息可交错，网页依据 `type` 和 `id` 分发。每条消息和 HTTP 请求体上限 1 MiB，WebSocket 最多同时 16 条。

浏览器接入示例：

```js
const scheme = location.protocol === "https:" ? "wss:" : "ws:";
const ws = new WebSocket(`${scheme}//${location.host}/ws`);
let nextId = 0;
const pending = new Map();

function command(method, params = {}) {
  const id = ++nextId;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

ws.onmessage = async ({ data }) => {
  const msg = JSON.parse(data);
  if (msg.type === "ready") {
    // 按网页登录流程获取凭据；robotd 凭据只配置在服务端。
    if (!msg.authenticated) await command("auth", webpageToken);
    console.log("网页遥控连接已就绪");
  } else if (msg.type === "reply") {
    const waiter = pending.get(msg.id);
    if (!waiter) return;
    pending.delete(msg.id);
    if (msg.error) waiter.reject(new Error(`${msg.error.code}: ${msg.error.message}`));
    else waiter.resolve(msg.result);
  } else if (msg.type === "state") {
    console.log("机器人状态", msg.state);
  } else if (msg.type === "backend_error") {
    console.error("robotd 连接故障", msg.error);
  }
};
ws.onclose = () => {
  for (const waiter of pending.values()) waiter.reject(new Error("遥控连接已断开"));
  pending.clear();
};

// 网页连接就绪后，由启用控制按钮调用；以下默认用于模拟设备。
async function enableControl() {
  await command("acquire", { owner: "browser", ttl_ms: 1000 });
}
// 网页在持有控制权且仍允许控制时，每约 300 ms 主动调用。
async function renewControl() { await command("renew"); }
// 摇杆只在当前按住/拖动时持续发送实时输入，建议每 50 ms 一次。
async function drive(speed, steering) {
  await command("targets", { targets: { chassis: {
    mode: "ACKERMANN", speed_m_s: speed, steering_rad: steering, ttl_ms: 200
  } } });
}
async function disableControl() { await command("release"); }
async function stopRobot() { await command("stop", { latch: true }); }
```

## 控制生命周期

网页必须显式 renew，状态推送、WebSocket ping/pong 和下发 targets 都不会续期租约。网页失活后由 robotd 租约到期停止全部设备；底盘目标自身的 TTL 100..1000 ms 独立计时，只有新的速度指令才能更新。网关不缓存或自动重发运动目标。

WebSocket 断开立即请求释放该连接的控制权；网关正常退出释放自身 HTTP/WS 会话。释放失败或进程被强制终止时，robotd 租约到期停止。网页失焦、切换后台、摇杆释放时应主动发送零速度或 release/stop，并停止发运动目标；重新取得控制权应由用户主动操作。

`stop` 撤销控制权，`latch=true` 还会锁定停止；重新 acquire 必须显式 `clear_latch=true`。有效配置更新同样撤销旧控制权。请求超时表示执行结果可能不确定，应查询状态，不重试运动命令。robotd 恢复后网关继续推送状态，但不自动 acquire 或恢复旧目标；网关重启也不恢复临时会话。

## 部署和验证

[robot-remote.service](deploy/robot-remote.service) 是独立 systemd 单元。`/etc/robot-remote.env` 可配置两个 token；服务不管理 robotd 生命周期，后端暂时离线时 HTTP 报错、WebSocket 推送 backend_error，后端恢复后继续工作。

```sh
python -m unittest discover -s robot_remote/tests -v
python -m unittest discover -s robotd/tests -v
python -m build --wheel --no-isolation
```

测试使用实际本机 HTTP/WebSocket/gRPC 连接和模拟设备，覆盖凭据、来源、配置版本冲突、目标校验、独占控制权、租约/底盘 TTL、断线/退出释放、动作以及后端离线恢复。
