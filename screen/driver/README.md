# FL7707N 双 DSI 点亮实验

用于 Raspberry Pi 5 和当前转接板，屏幕型号 HD40031C30-Y，720×720。
初始化命令来自上级目录厂家 `2lane V5.5` TXT，保持原寄存器内容和延时。
供电由转接板提供，驱动只控制复位和 DSI，不管理背光电源。

## 2026-10-19 实机结果

设备 `ip-rpi`，Debian 13，内核 `6.18.50+rpt-rpi-2712`。

- 模块和设备树均在设备本机编译成功。
- 运行时加载设备树成功，未修改 `/boot/firmware/config.txt`。
- CAM/DISP0：RP1 GPIO34 复位，`/dev/dri/card2`，`DSI-1`，`/dev/fb0`。
- CAM/DISP1：RP1 GPIO46 复位，`/dev/dri/card3`，`DSI-2`，`/dev/fb1`。
- 两路均报告 connected，模式为 720×720@60，CRTC active=1。
- 时序为 H=720/120/120/120，V=720/16/4/16，像素时钟 48.989 MHz。
- 两路各 23 条初始化命令发送完成；`0x0a` 回读均返回 `-5`，不能据此确认屏幕已接收命令。
- 已停止 display-manager，向两块 framebuffer 分别写入彩条与大号 0/1。
- 首轮现场反馈：两块均不亮。

### 完整转接板原理图补充后的修正

完整图 `SCH_hat_test_2026-10-19.pdf` 确认：LCD1_PWM/LCD2_PWM
直接控制 U18/U19（SY7200AABC）的 EN/PWM，不能只发送厂家初始化表。
当前驱动保留原始表，在发送时做两处板级调整：

- C7 参数 1 从 0x10 加上 bit7，开启 PWM 输出。
- C8 参数 1/2 调为 0x00/0x03，PWM 频率约 35 kHz；SY7200A 推荐范围为 20 kHz 至 1 MHz。

初始化后追加 `51 80 80`（约半亮度）和 `53 24`（BCTRL/BL 开启）。
回读前显式设置最大回包长度，并新增 0x04 面板 ID 回读。
单路热加载试验中，ID 和状态仍均返回 -5；不能把写入完成当作背光已亮。
热加载后 DRM 编号发生变化，已重启后干净复测：两路都 enabled，均已
执行 PWM 开启和亮度命令，但 ID/状态回读仍均为 -5。
`show-pattern.py` 已补充显式 framebuffer modeset，使没有控制台的第二路也能启用。
最新日志为远端 `dmesg-pwm-cleanboot.log`；新版实际画面等待现场确认。

芯片资料：[Silergy SY7200A](https://www.silergy.com/zh/download/downloadFile?ftype=note&id=4351&type=product)。

DRM 的 connected 是固定面板驱动报告的状态，不是线缆或面板应答检测。
DRM 卡号可能随启动顺序变化，重新启动后用 `kmsprint` 核对。

远端源码和记录位于 `/home/iprpi/screen-bringup/`：
`driver/`、`build.log`、`dmesg-first-test.log`、`drm-dsi0-state.txt`、`drm-dsi1-state.txt`。

## 编译与运行时加载

在本机重新生成初始化表：

```powershell
python screen/driver/generate-init.py
```

把 `driver` 目录复制到树莓派，在该目录执行：

```sh
make -j4
sudo install -D -m 644 panel-fl7707n.ko /lib/modules/$(uname -r)/extra/panel-fl7707n.ko
sudo depmod -a
sudo modprobe panel-fl7707n
sudo dtoverlay -d . fl7707n-dual
kmsprint --device=/dev/dri/card2 -m
kmsprint --device=/dev/dri/card3 -m
```

显示测试图时先释放桌面占用，再执行：

```sh
sudo systemctl stop display-manager
sudo python3 show-pattern.py
```

`show-pattern.py` 使用 32 位 XRGB framebuffer；本次实机 stride 为 2880。
停止桌面服务耗时较长时，可以用 `sudo systemctl --no-block stop display-manager`，
再检查 `systemctl is-active display-manager` 已变为 inactive。

## 后续接入眼屏应用

当前 Pi 5 把两路 DSI 暴露为不同的 DRM 设备，可使用已有 eyes 应用的双设备参数：

```sh
./eyes --headless --drm-device /dev/dri/card2 --left-connector DSI-1 \
  --right-drm-device /dev/dri/card3 --right-connector DSI-2 \
  --socket /tmp/eyes.sock --fps 60
```

此命令尚未在本次设备运行。先确认彩条图，再部署渲染应用。

## 恢复与启动配置

本次只动态加载了屏幕设备树，重启即可恢复原启动配置。
如果只想恢复桌面服务：

```sh
sudo systemctl start display-manager
```

确认点亮后，如需开机加载，先备份配置，再安装 overlay：

```sh
sudo cp -a /boot/firmware/config.txt /boot/firmware/config.txt.before-fl7707n
sudo install -m 644 fl7707n-dual.dtbo /boot/firmware/overlays/fl7707n-dual.dtbo
```

在 `config.txt` 的 `[all]` 段加入 `dtoverlay=fl7707n-dual`。
模块通过设备匹配自动加载；内核更新后需为新内核重新编译安装模块。
设备树将两个相机稳压器节点设为 disabled，避免它们占用同一根复位脚。
