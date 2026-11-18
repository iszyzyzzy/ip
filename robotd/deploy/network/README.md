# 树莓派网络恢复与遥控热点

依赖 Raspberry Pi OS 的 NetworkManager、Netplan、python3-yaml 和 dnsmasq。
在客户端 Wi-Fi 已连接时安装：

```sh
sudo sh install.sh
```

更新脚本与服务时也运行此命令，保留已有热点密码、模式和恢复基线。

安装自动备份当前可用的 `/etc/netplan/*.yaml` 到
`/var/lib/robot-network/netplan/`。开机在 NetworkManager 启动前检查配置；
运行中每 30 秒检查一次。空配置、格式错误或 Netplan 生成失败时恢复基线，
异常文件留在 `/var/lib/robot-network/damaged-时间/`。

恢复基线不随周期检查更新，避免错误配置覆盖备份。
改过客户端网络并确认 Wi-Fi 连通后，手动更新基线：

```sh
sudo robot-network backup
```

## 调试与交付

默认 `fallback`：优先使用原有有线或无线连接，连续失联 90 秒后开启热点。
检查间隔为 30 秒，开机首次检查在 20 秒后，首次启动的救援热点通常在约两分钟内出现。
有可用局域网地址就视为连通，不要求互联网访问。
热点开启后保持，避免维修期间断开手机。

交付使用常驻热点模式，下次检查生效，当前客户端 Wi-Fi 会断开；重启后仍为热点：

```sh
sudo robot-network mode hotspot
```

热点名 `ip-rpi-control`，2.4 GHz / WPA2，固定地址 `192.168.50.1`。
密码在设备上随机生成，可通过 SSH 查询：

```sh
sudo sed -n 's/^psk=//p' /var/lib/robot-network/hotspot.nmconnection
ssh iprpi@192.168.50.1
```

手机需保留这个没有互联网的 Wi-Fi 连接。网页遥控网关若监听 `0.0.0.0:8080`，
地址为 `http://192.168.50.1:8080/`；网关进程需单独启动。

恢复客户端模式：

```sh
sudo robot-network mode fallback
sudo nmcli connection up netplan-wlan0-Lab107
```

连接热点时执行第二条会断开热点，可重新连接 `Lab107` 后访问树莓派。
热点模式保留以太网连接能力。热点配置每次从独立基线加载到 `/run`，
不依赖 Netplan 的 `90-NM` 文件。

查看日志与停用：

```sh
sudo journalctl -u robot-network-prepare -u robot-network --no-pager -n 50
sudo systemctl disable --now robot-network.timer
sudo systemctl disable robot-network-prepare.service
```

## 已知问题

恢复服务用于配置丢失或损坏后的连入；SD 卡、文件系统或系统无法启动时仍需离线修复。
树莓派单个 `wlan0` 在客户端与热点间切换，不能同时连接 Lab107 和提供此热点。

2026-11-10 已在 Pi 5 实测进入 AP 模式、配置 `192.168.50.1/24`、启动 DHCP、
保持 SSH 监听并自动返回 Lab107；配置损坏恢复和切换条件通过 5 项隔离测试。
尚未做手机入网、网页遥控及重启验证。
