# Pi 5 + CMSIS-DAP + CAN1 底盘准备

接线：DM-MC02（STM32H723VGT6）USB CDC与CMSIS-DAP USB都接树莓派，调试器SWD接板卡；RANGER AIR接CAN1（PD0 RX / PD1 TX），500kbps经典CAN。CAN H/L与地按板卡接口连接，确认终端电阻。颈部电机当前禁用：底盘500kbps与颈部1Mbps不能共用CAN1。

## 上线后识别与烧录

```sh
sudo systemctl stop robotd
lsusb
ls -l /dev/serial/by-id/
openocd --version
# 如未安装：sudo apt install openocd python3-serial usbutils
# 先确认调试器唯一、目标确为STM32H723；本例使用板上SWD接口。
sh flash-pi.sh /绝对路径/firmware.hex "$HOME/h7-backups"
```

`flash-pi.sh`使用CMSIS-DAP、SWD 1MHz和STM32H7配置；先打印芯片/Flash信息、备份0x08000000起1MiB，再烧录、校验、复位。没有整片擦除选项，不改option bytes。若复位连接失败，先检查SWD、GND、目标供电和NRST，不自动提高时钟或解除读保护。备份命令或验证失败时不报告成功。USB权限不足时配置udev规则或以sudo运行OpenOCD；不要并行运行其他调试进程。

固件上电默认桥接通道为1Mbps，底盘工具/robotd打开时显式设置CAN1 500kbps。颈部控制在未被CDC位置命令认领前不会主动控制电机，诊断工具不发送颈部或底盘运动命令。

## 先采集反馈

将`chassis_check.py`和`dm_can.py`放在同一目录；保持robotd停止以独占CDC。

```sh
python3 dm_can.py ports
python3 chassis_check.py --port /dev/serial/by-id/实际H723路径 --channel 1 --seconds 15 --output chassis-check.jsonl
```

默认只听，不发送CAN ACK或控制帧。如果总线拓扑需要外部节点ACK，可加`--normal`，仍不发送运动/模式切换帧。退出会恢复之前的CAN配置。记录原始帧、CAN统计、上位机丢帧和协议错误，并解码0x211系统状态、0x221运动反馈、0x291模式反馈；没有识别到底盘反馈时退出码为2，不把USB枚举成功当作底盘已通。

先确认电压、故障位、CAN控制权限和反馈连续性，再通过robotd短时低速测试。robotd配置使用`chassis.channel=1`、`bitrate=500000`、`neck.enabled=false`，H7端口用`/dev/serial/by-id/`。配置仍由robotd RPC保存，不直接修改数据库。动作TTL、急停和模式切换确认沿用robotd现有控制流程；本次离线准备不启用开机运动。
