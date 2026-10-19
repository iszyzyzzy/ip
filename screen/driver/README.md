# FL7707N 双 DSI 点亮实验

用于 Raspberry Pi 5 和当前转接板，屏幕型号 HD40031C30-Y，720×720。
初始化命令来自上级目录厂家 `2lane V5.5` TXT，原始表保持不变；
运行时针对本转接板调整 PWM 配置和显示、背光延时，见下方最新核查记录。
供电由转接板提供，驱动控制复位、DSI 及屏幕输出到背光芯片的 PWM。

## 2026-10-20 时序修正及资料核查

**最新实测更新：** 见 [LP-11 与回读恢复记录](AUDIT-LP11-20261020.md)。
仅设置 prepare_prev_first 并不能保证时钟 LP-11；加入 MIPI_DSI_CLOCK_NON_CONTINUOUS 后，
首次成功读到 ID `93 65 04`、复位状态 `08`、Sleep Out 后状态 `18`。
无需修改主机接收计时、ECC/CRC 寄存器即可回读，不能再把 -5 归因于缺少读等待配置。
用户反馈仍无背光或画面。当前 C7、51 在 Command Mode 配置，先发 `53 20` 再写亮度；
视频启动 50 ms 后仅发 `53 24` 短包开启 BL。这取代下方上一轮方案中的 Video Mode 重写 C7。

万用表测量模式通过模块参数 `meter_reset_ms` 开启，0 为普通初始化，最大每阶段 30000 ms：

```sh
echo 30000 | sudo tee /sys/module/panel_fl7707n/parameters/meter_reset_ms
sudo python3 trigger-init.py --fb /dev/fb0
```

测量模式在复位前强制撤销 HS 时钟请求并检查 CLK/D0/D1 Stop 位；RST 依次低、高、再低，
每阶段保持指定毫秒数，最后按正常顺序初始化并恢复自动非连续时钟。
停止后设回 0，下次初始化恢复普通速度。冷启动配置未修改。

核查详情见 [AUDIT-20261020.md](AUDIT-20261020.md)。以下描述取代旧实验记录中的提前开背光流程：

1. RST 拉低 20 ms，释放后等待 120 ms，再发送厂家初始化表。
2. Sleep Out 后等待 250 ms；Display On 后等待 150 ms（厂家表为 50 ms）。
3. prepare 阶段保持 C7.PWM_OE=0；RP1 启动视频后，在 panel enable 中再等 50 ms，
   发送 `51 80 80`、`53 24`，最后打开 C7.PWM_OE。
4. panel disable 先关闭 PWM 和背光控制，之后再停视频、Display Off、Sleep In、复位。

这样以明确延时满足现场图中的 Display On 后 >120 ms、视频启动后背光延迟 >33.2 ms，
不依赖寄存器回读失败产生的等待。该图来自 ST7701S 建议资料，并非 FL7707N 专用规格；
FL7707N 第 195 页另外要求释放复位前 DSI 数据线及时钟线处于 LP-11，
当前通过 prepare_prev_first 让 RP1 DSI 主机先初始化。实际引脚时序仍以示波器为准。

两路 1.8 V/2.8 V 来自常供电 LDO，驱动不能改变它们的上升先后顺序。
本次只在运行时加载单路 CAM/DISP0；未新增开机配置。

完整图放大核对后确认，U18/U19 的 OVP 分别接 D4/D5 阴极（升压输出 LCD_A），
与 5 V 输入线交叉处无连接点。此前怀疑 OVP 接到输入端属于图纸判读错误，已撤回。

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
./eyes --headless \
  --drm-device /dev/dri/by-path/platform-1f00118000.dsi-card --left-connector DSI-1 \
  --right-drm-device /dev/dri/by-path/platform-1f00130000.dsi-card --right-connector DSI-2 \
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

受控重启实测通过：`screen-pattern` 和 `screen-monitor` 均自动进入 active，
两路 RST 自动为高，两块 framebuffer 的彩条均自动写入。
boot ID 从 `dab95a48-601f-4b43-b2af-53e861022925` 变为
`77f493e5-3bdf-43d0-9e8f-9f91c1b37449`，上一轮 journal 保存了完整的正常重启尾部。
开机提前加载改变了 DRM 卡号，本次两路为 card0/card1；上面的 eyes 命令
使用已核实的 `/dev/dri/by-path/` 路径，避免依赖编号。

受控重启后连续观察至 uptime 339 秒，12 条区间报告均属于同一个 boot ID，
欠压/限频标志始终为 0x0，2 秒采样记录的 EXT5V 范围为 5.061179–5.09736 V。
两路 RST 保持高、DSI 保持 enabled，监测服务 NRestarts=0，未发现内核欠压、
panic、Oops 或锁死日志。该轮未观察到异常重启，监测服务继续后台运行。
不能据此排除更短瞬态或确定用户此前 1.5 V 读数是仪表误差。

模块安装针对当前内核，内核更新后需重新编译安装。
设备树将两个相机稳压器节点设为 disabled，避免占用同一根复位脚。

## 回退

2026-10-19 后续现场恢复记录：设备无法通过原 IP 或主机名连接，用户把 microSD
插入 Windows。已确认 E: 为 FAT32 bootfs，当前 config 与安装前备份仅差新增的
双屏 overlay 段。离线将 config.txt 恢复为 config.txt.before-fl7707n 的原始字节，
并在 cmdline.txt 的同一行末尾加入：

```text
systemd.mask=screen-pattern.service systemd.mask=screen-monitor.service
```

这会在后续启动中临时屏蔽两项新增服务。原始 cmdline 的 root PARTUUID、网络相关
参数均保留；未修改网络文件。修改前文件备份在本机
`screen/bringup/recovery-20261019-211645/`，config 回读 SHA256 与安装前备份一致。
等待用户安全弹出、插回设备并上电确认网络；此时尚不能认定离线原因。
恢复连接后先查持久化 journal，再决定是否重新启用显示实验。

以下命令适用于设备仍可通过 SSH 访问的情况：

```sh
sudo systemctl disable --now screen-pattern screen-monitor
sudo cp -a /boot/firmware/config.txt.before-fl7707n /boot/firmware/config.txt
sudo rm -f /etc/modules-load.d/fl7707n.conf
sudo systemctl enable lightdm.service
sudo reboot
```

此回退恢复安装前的启动配置；如之后对 config.txt 另有修改，应只删除
`dtoverlay=fl7707n-dual`，避免覆盖后续修改。持久化日志配置保留以供排查。

## 黑屏与 PWM 低电平的波形定位

重刷后重新测试时，新增 `fl7707n-cam0-overlay.dts`，只启用 CAM/DISP0。
源码在设备本机重新编译通过，用 `sudo insmod ./panel-fl7707n.ko` 和
`sudo dtoverlay -d . fl7707n-cam0` 首次加载产生初始化波形，未部署开机配置。
本轮首次加载已确认执行：GPIO34 高、GPIO46 低，DSI-1 enabled，SSH 正常，
欠压标志为零；面板 ID 和状态回读仍均为 -5。等待现场示波器捕获结果。
本轮没有通过 framebuffer 关显/恢复再次触发。

现场反馈：屏幕 FPC 的 VCC/IOVCC 正常，PWM 近低电平，背光 A 约 4.9 V。
该现象与背光未获得使能相符，不能单独认定升压芯片损坏。
驱动日志中的背光配置发送完成，也不等于面板已经接收和执行。

先抓 CAM/DISP0 对应屏幕的 RST 与 D0+，而不是继续猜测视频时序：

1. CH1 接屏幕侧 RST，CH2 接屏幕侧 D0+，探头 10×，地夹均接 GND。
   D0− 是信号线，不能作为普通探头地。
2. 单次触发设 CH1 上升沿约 0.8 V，先用 50 ms/div、尽量大的存储深度。
3. 示波器就绪后，在树莓派执行一次：

   ```sh
   sudo python3 ~/screen-bringup/driver/trigger-init.py --fb /dev/fb0
   ```

4. 代码会关闭这一路、保持复位，再恢复输出。RST 释放后等待 120 ms 才发送初始化表。
   确认初始命令活动后，再放大查看 D0+ 的低速脉冲。

判断顺序：RST 未到屏幕，查复位路径；RST 正常但屏幕侧没有命令活动，
对照树莓派端与屏幕端 D0+/D0− 的波形及断电通断；命令活动到达但无回读，
继续核对 D0 极性、完整链路针脚映射与面板实际型号。
只有收到配置后仍不产生 PWM，才进一步集中检查 PWM 配置与输出连接。

`fb0` 已核实为 `/sys/devices/platform/axi/1000120000.pcie/1f00118000.dsi`，
即 CAM/DISP0；`fb1` 为 `1f00130000.dsi`，即 CAM/DISP1。
