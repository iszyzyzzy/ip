# 树莓派5双眼屏部署实测

2026-10-20，目标 `iprpi@192.168.3.109`，Debian 13 ARM64，
内核 `6.18.50+rpt-rpi-2712`。两块屏幕均已由用户确认彩条正常。
面板使用 `screen/driver` 中完整 Waveshare 4.0C JD9365 配置，
两通道 RGB888，720×720，DRM 显示模式约60Hz。

## 部署目录与独立运行

源码：`/home/iprpi/eyes`；发布包：`/home/iprpi/eyes/dist/eyes-linux`。
运行日志：`/home/iprpi/screen-bringup/eyes-runtime.log`。

驱动已加载、桌面已停止时，从源码目录运行：

```sh
sh scripts/run-pi5.sh
```

当前采用 Idle 模式，独立注视和眨眼，gRPC/UDP监听仍运行，不依赖手机。
配置文件位于发布包的 `eyes-settings.json`，手机IP已存为 `192.168.3.110`，
但本轮按用户要求没有发起面捕连接。进程当前在后台运行。
脚本支持作为参数传入手机IP，届时才切换Face模式并发起握手：

```sh
sh scripts/run-pi5.sh 192.168.3.110
```

两种启动方式选择一种；切换前先终止当前眼屏进程，避免争用DRM和控制socket。
启动脚本会在退出时终止自身启动的子进程。未创建或启用开机服务，
未修改 `/boot/firmware/config.txt`，其SHA-256在部署后校验一致。

## 实机验证

- 原生ARM64 release构建成功，Rust 1.99.1，保留原Cargo.lock。
- 为缩短板端构建时间，本轮 `CARGO_BUILD_JOBS=3 CARGO_PROFILE_RELEASE_LTO=false`。
- 原依赖源下载慢，构建时使用rsproxy镜像；安装器与官方文件SHA-256一致。
- `--list-drm` 在两路分别列出独立CRTC、连接器及720×720@60首选模式。
- 日志明确选择硬件适配器 `V3D 7.1.10.2`，程序无需X11/Wayland桌面。
- gRPC实测 `output_mode=drm`、`config.mode=0`；两路输出描述分别为DSI-1/DSI-2。
- 内核DRM状态实测两路 `active=1`，扫描缓冲均 `allocated by = eyes`、尺寸720×720。
- 状态返回Idle注视姿态，配置保存为 `Idle`；运行时温度约49°C，throttled=0。
- `--fps 60`是目标循环帧率，尚未测量实际渲染/翻页帧率。
- 当前屏幕实际动画效果待用户现场确认；手机面捕接收尚未验证。

稳定设备路径：

| 屏幕 | DRM设备 | 连接器 |
| --- | --- | --- |
| CAM/DISP0 / 左眼 | `/dev/dri/by-path/platform-1f00118000.dsi-card` | DSI-1 |
| CAM/DISP1 / 右眼 | `/dev/dri/by-path/platform-1f00130000.dsi-card` | DSI-2 |

查询状态：

```sh
dist/eyes-linux/controller /run/user/1000/eyes.sock state
```

## 本次修复

首次实机启动时，wgpu默认设备请求要求二维纹理上限8192，V3D上限7680，
导致 `LimitsExceeded(max_texture_dimension_2d)`。离屏渲染设备现在采用
`Limits::downlevel_defaults().using_resolution(adapter.limits())`，
减少不需要的默认限额请求，并采用实际适配器的纹理尺寸上限。
修复后在同一硬件上设备创建、GPU渲染和双路DRM提交均成功。

## 2026-10-21 重启日志与面捕试运行

用户确认独立渲染正常，随后报告发生重启。重连时设备已启动约28分钟，
渲染与运行时面板模块均未自动恢复。上次启动日志不可用：树莓派系统自带
`/usr/lib/systemd/journald.conf.d/40-rpi-volatile-storage.conf` 设置了 `Storage=volatile`。
本次启动未找到欠压、存储错误或内核崩溃证据；日志中的硬件看门狗启动记录
不能直接当作上次重启原因。

已单独运行 `screen/driver/install-logging.sh`，覆盖为持久日志、上限128MiB、
同步间隔5秒，并启用 `screen-monitor.service` 开机采样供电、温度及复位状态。
日志落在根分区 `/var/log/journal`，确认实际生成system.journal。
突然掉电仍可能丢失尚未落盘的最后几秒，不能仅凭日志未报欠压排除供电瞬态。

双屏驱动仍手动加载。渲染改用本次启动内的临时服务，便于将输出写入持久journal：

```sh
sudo systemd-run --unit=eyes-render --collect --uid=iprpi \
  --working-directory=/home/iprpi/eyes \
  --setenv=HOME=/home/iprpi --setenv=XDG_RUNTIME_DIR=/run/user/1000 \
  --property=Restart=no /usr/bin/sh /home/iprpi/eyes/scripts/run-pi5.sh 192.168.3.110
```

该命令不设置眼屏开机自启。gRPC回读确认mode=2、phone_ip=192.168.3.110、
output_mode=drm，已发送启动请求；初次采样收到0包，等待手机端运行。

查询日志：

```sh
sudo journalctl -k -b -1
sudo journalctl -u screen-monitor.service -b -1
sudo journalctl -u eyes-render.service -b -1
```

gRPC的 `Configure` 可修改 `phone_ip` 和 `tracking_port`，`mode=2` 为面捕。
改地址不会自动发包，需再调用 `ConnectTracking`；`SaveSettings` 保存配置。
超过1秒未收到有效面捕数据时，程序继续独立注视及自动眨眼，DRM渲染与gRPC不中断。

### 手机打开后重发连接请求

用户在请求后打开手机，再次调用ConnectTracking后，实测tracking_live=true，
有效包从715递增至932，rejected_packets=0，左右眼开合、视线和头部滚转姿态均更新。
当前连接请求是单次发送，未实现未连接时自动重试。

发现收包已正常但tracking_status仍保留“等待数据”的旧提示，已修复为随有效包更新，
超时显示数据超时，恢复收包后重新显示接收状态。原生ARM64的3项面捕测试通过，
其中UDP回环覆盖收包、超时及恢复；release重编译通过后替换部署并重启本次临时服务。
