# 网页遥控器 web_remote_control

Vue 3 + shadcn-vue 的手机优先遥控页面，通过 WebSocket 连接 [robot-remote](../robot_remote/README.md) 网关。配置统一由 robotd 保存，页面不建立另一份配置。

| 页面 | 功能 |
| --- | --- |
| 底盘 | 阿克曼 / 斜移 / 自旋 / 驻车，摇杆按住时每 50 ms 发送、松手补发零速度；档位缩放速度 |
| 头部 | 位置摇杆映射到颈部限位，可选眼睛跟随；眨眼、回中、设为零点 |
| 手动 | 左右臂各4颗舵机，显示实际位置；滑条松手或数字输入后发送单舵机位置，固定T0001，另有±10微调 |
| 眼屏 | 自主/面捕/定睛，手机IP与连接，眼睛/虹膜/瞳孔大小、表情、颜色、眨眼与弹性；应用并保存 |
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

## 手动舵机与眼屏参数

手动页使用原始500～2500位置读数，不改标定零点，不向其他舵机补发位置。
滑条仅预览目标，松手时发送；数字输入后点击移动。±10基于最新实际位置微调。
需要控制权及2秒内有效反馈；命令超时不自动重试。
`targets.servo`字段为`right_arm`、`joint_index`（根部起0～3）、`position_pwm`；
robotd将它转换为对应ID的`#001PxxxxT0001!`，原有整臂角度接口保持不变。
原始位置调试范围与关节角度标定分开，机械活动余量由操作者确认。

眼屏页通过带版本检查的UpdateConfig保存参数；应用前释放控制，避免配置重连时仍有运动。
草稿期间若其他客户端更新配置，需重新载入，防止覆盖他人的修改。
选择面捕并应用或点击连接面捕时，robotd配置渲染器后调用ConnectTracking；
手机不在线时不会阻断独立渲染，页面显示实际接收状态和包数，手机就绪后可再次点击连接。
当前Pi面捕地址192.168.3.110，UDP49983。

眼屏脚本支持`EYES_SOCKET`，可将控制socket放到robotd能够访问的共享运行目录。
实机使用`/run/robot-eyes/control.sock`，父目录组robot/权限2750，进程UMask0007，
避免依赖用户私有的`/run/user/1000`。服务仍为本次运行的临时服务，没有新增开机配置。
