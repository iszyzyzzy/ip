# 网页遥控器 web_remote_control

Vue 3 + shadcn-vue 的手机优先遥控页面，通过 WebSocket 连接 [robot-remote](../robot_remote/README.md) 网关。页面不保存配置，限位从 robotd 配置读取。

| 页面 | 功能 |
| --- | --- |
| 底盘 | 阿克曼 / 斜移 / 自旋 / 驻车，摇杆按住时每 50 ms 发送、松手补发零速度；档位缩放速度 |
| 头部 | 位置摇杆映射到颈部限位，可选眼睛跟随；眨眼、回中、设为零点 |
| 动作 | 列出、执行、取消 robotd 动作 |
| 状态 | 控制者、租约、电池、电机与最近事件 |

底部急停无需控制权，发送 `stop(latch=true)`；锁定后需点击“解锁并控制”重新取得控制权。页面每 300 ms 续期 1 s 租约，切到后台或关闭页面时主动释放。

## 开发

```sh
npm install
# 另开终端启动 robotd --simulate 和 robot-remote（默认 :8080）。
npm run dev -- --host
```

开发服务器把 `/ws`、`/api`、`/healthz` 代理到 `ROBOT_REMOTE_URL`（默认 `http://127.0.0.1:8080`），与部署同源，无需 `--allow-origin`。手机与电脑处于同一局域网时访问终端打印的 Network 地址。

## 部署

```sh
npm run build
robot-remote --web-root web_remote_control/dist --host 0.0.0.0
```

将 `dist` 复制到树莓派后在 [robot-remote.service](../robot_remote/deploy/robot-remote.service) 的 `ExecStart` 加入 `--web-root <目录>`。网关设置了 `ROBOT_REMOTE_API_TOKEN` 时页面会提示输入，凭据只保存在该浏览器 localStorage。明文 HTTP 下凭据和指令未加密，仅在可信局域网使用，或经 HTTPS 反向代理访问。

`src/components/ui` 由 `npx shadcn-vue add <组件>` 生成；Switch 已把 `data-checked:` 改成 `data-[state=checked]:` 以匹配 reka-ui 的属性。
