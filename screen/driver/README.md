# FL7707N 双 DSI 点亮实验

用于 Raspberry Pi 5 和当前转接板，屏幕型号 HD40031C30-Y，720×720。
初始化命令来自上级目录厂家 `2lane V5.5` TXT，保持原寄存器内容和延时。
供电由转接板提供，驱动控制复位、DSI 及屏幕输出到背光芯片的 PWM。

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

### 供电实测及图纸判读更正

此前将辅助电源 EN 连线误判为接地，该判断已撤回。用户确认 EN 接到 5 V，
且实测 1V8=1.80 V、2V8=2.81 V、5V=4.96 V，供电电压正常。
用户已断电更换一套 FFC 线和延长器，继续在这套接线下复测。

换线后已重新连接设备，加载当前模块与 overlay，并运行 `show-pattern.py`。
两路均为 enabled、720×720，复位 GPIO34/46 均为高电平，系统欠压标志为零；
两路 0x04 ID 和 0x0A 电源状态回读仍返回 -5。日志保存为远端
`/home/iprpi/screen-bringup/dmesg-new-ffc.log`。
画面等待现场确认；若仍全黑，优先测屏幕连接器处的 RST 和 PWM 实际信号，
并确认电源是否经过 FFC/延长器送到屏幕侧。

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

## 开机启动及供电监测

用户要求改为开机加载，并排查可能的异常重启。执行 `sudo sh install-boot.sh` 后：

- 首次备份启动配置到 `/boot/firmware/config.txt.before-fl7707n`，安装 overlay 并在 `[all]` 中启用。
- `/etc/modules-load.d/fl7707n.conf` 确保自定义面板模块加载。
- `screen-pattern.service` 启用两路 framebuffer 并写入 0/1 彩条。
- `screen-monitor.service` 每 2 秒采样 PMIC 的 EXT5V、欠压/限频标志；每 30 秒记录区间极值、温度、RST 和 DSI 状态，标志变化时立即记录。
- journal 持久化保存，最大 32 MB、同步间隔 15 秒；能保存后续启动的日志。
- 测试期间禁用原先启用的 `lightdm.service`，避免桌面接管彩条。

监测只读状态，不自动重启系统。启动 ID 和 uptime 用于区分重启和时钟校正。
2 秒采样不能排除更短的电源瞬态，PMIC 电压是树莓派输入，不是转接板 1V8/2V8。
极端断电可能丢失尚未落盘的最后几条记录。

```sh
systemctl status screen-pattern screen-monitor
sudo journalctl -u screen-monitor -f -o cat
sudo journalctl --list-boots
sudo journalctl -b -1 -e
```

首次检查时系统原先没有持久化 journal，无法还原用户所述红灯前的过程。
当次读到 `get_throttled=0x0`、`get_rsts=1000`、`power_reset=00000000`，
PMIC 输入约 5.075 V；这些只描述当次启动，不能反推前一次异常的原因。
运行中的硬件 watchdog 超时为 1 分钟，保持原设置不变。

模块安装针对当前内核，内核更新后需重新编译安装。
设备树将两个相机稳压器节点设为 disabled，避免占用同一根复位脚。

## 回退

```sh
sudo systemctl disable --now screen-pattern screen-monitor
sudo cp -a /boot/firmware/config.txt.before-fl7707n /boot/firmware/config.txt
sudo rm -f /etc/modules-load.d/fl7707n.conf
sudo systemctl enable lightdm.service
sudo reboot
```

此回退恢复安装前的启动配置；如之后对 config.txt 另有修改，应只删除
`dtoverlay=fl7707n-dual`，避免覆盖后续修改。持久化日志配置保留以供排查。
