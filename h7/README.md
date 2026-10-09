# 达妙 H723 USB CDC 转三路 CAN

用于达妙 DM-MC02 / DM-MC-Board02（STM32H723VGT6）。单个 USB CDC 虚拟串口同时访问 CAN1、CAN2、CAN3，每路独立设置波特率和工作模式，三路总线之间不互相转发。

支持经典 CAN、CAN FD、标准/扩展 ID、经典 CAN 远程帧、FD BRS、只听模式和内部回环。上电三路均为经典 CAN、1 Mbps、正常模式；USB 上位机打开串口并置 DTR 后开始上报接收帧。设置仅保存在 RAM，复位后恢复默认配置。

## 硬件配置

引脚和时钟依据[达妙官方资料](https://github.com/dmBots/DM-MC02)，固定参考版本 `d65a3688981405dc205f628f1a026bde8cf8303e`。

| 接口 | RX / D− | TX / D+ | 复用 |
| --- | --- | --- | --- |
| CAN1 / FDCAN1 | PD0 | PD1 | AF9 |
| CAN2 / FDCAN2 | PB5 | PB6 | AF9 |
| CAN3 / FDCAN3 | PD12 | PD13 | AF5 |
| USB OTG HS 内置 FS PHY | PA11 | PA12 | 专用 USB 引脚 |

24 MHz HSE，CPU 480 MHz，AHB 240 MHz，FDCAN 内核 120 MHz。USB 使用 HSI48，并通过 USB SOF/CRS 校准。USB 运行在 **Full Speed 12 Mbps**；外设名中的 HS 不表示当前使用 USB High Speed。

经典 CAN 仲裁速率：50k、100k、125k、250k、500k、800k、1M bit/s，采样点 80%。FD 数据速率：1M、2M、3M、4M、5M、6M、8M bit/s，采样点 73.3%～75%；高速正常模式启用发送延迟补偿。实际 FD 数据速率需与总线节点、收发器和布线匹配。

连接板上 CAN_H/CAN_L 端子；总线两端各使用 120 Ω 终端，各节点共地。通过板卡规定的电源接口供电，SWD 烧录使用板上 SWD 接口。

## 编译和烧录

工程已经内置所需 STM32CubeH7 v1.12.1 HAL、CMSIS 和 USB CDC 库；不需要 CubeMX 生成代码，也不需要联网下载依赖。使用 ARM GNU、CMake 和 Ninja：

```powershell
cd D:\code\ip\h7
.\build.ps1

# ARM 工具链尚未加入 PATH 时：
.\build.ps1 -ArmGccBin 'C:\Program Files (x86)\Arm GNU Toolchain arm-none-eabi\12.3 rel1\bin'
```

产物：`build/firmware.elf`、`build/firmware.hex`、`build/firmware.bin`、`build/firmware.map`。Flash 起始地址 `0x08000000`，无自定义 bootloader。链接脚本使用 1 MB Flash、128 KB AXI SRAM，并在 DTCM 顶部使用主栈；8 KB 为栈预留预算。

STM32CubeProgrammer 选择 ST-LINK/SWD，打开 `firmware.hex`，下载并校验，随后复位。命令行示例（实际执行时替换为本机工具路径）：

```powershell
& 'C:\Program Files\STMicroelectronics\STM32Cube\STM32CubeProgrammer\bin\STM32_Programmer_CLI.exe' -c port=SWD -w .\build\firmware.hex -v -rst
```

若使用 BIN，下载地址应为 `0x08000000`。USB 枚举为 `DM H723 USB CAN x3`，沿用 ST CDC 示例 VID/PID `0483:5740`，序列号来自 MCU UID。Windows 10/11 使用 USB 虚拟串口驱动。

## 上位机使用

```powershell
python -m pip install -r tools\requirements.txt
python tools\dm_can.py ports
python tools\dm_can.py --port COM7 info
python tools\dm_can.py --port COM7 stats

# 设置 CAN2 为经典 CAN 500 kbps；其他通道不受影响。
python tools\dm_can.py --port COM7 config --channel 2 --bitrate 500000

# 发送标准帧，并等待总线发送完成确认。
python tools\dm_can.py --port COM7 send --channel 2 --id 0x123 --data 0102030405060708

# 设置 CAN3 为 FD，仲裁 1 Mbps、数据 2 Mbps。
python tools\dm_can.py --port COM7 config --channel 3 --fd --data-bitrate 2000000
python tools\dm_can.py --port COM7 send --channel 3 --id 0x123 --fd --brs --data 0102030405060708090a0b0c

# 同时监控全部三路，逐行输出 JSON。
python tools\dm_can.py --port COM7 monitor
```

虚拟串口设置中的 `115200` 仅用于 CDC line coding，不决定 USB 或 CAN 的传输速率。打开串口时必须置 DTR；工具会自动处理。一次只由一个程序占用串口。

可直接导入 Python API，持续打开串口构建自己的上位机：

```python
import sys
sys.path.insert(0, r'D:\code\ip\h7\tools')
from dm_can import Bridge

with Bridge('COM7') as can:
    can.configure(1, nominal=1000000)
    can.configure(2, nominal=500000)
    can.configure(3, nominal=1000000, data_rate=2000000, fd=True)
    can.send(1, 0x123, bytes.fromhex('01020304'))
    frame = can.recv(timeout=1.0)
    if frame is not None:
        print(frame.as_dict())
```

`Bridge` 是单线程同步接口；等待命令响应时会缓存全部三路收到的 CAN 帧。异步或高吞吐上位机可以直接实现 [协议](docs/protocol.md)：每路最多 16 个在途发送，ACK 表示入队，TX_RESULT 表示实际发送结果。超时不会自动重发。

## 队列与诊断

- 每路硬件 RX FIFO 16 帧、硬件 TX FIFO 16 帧、TX event FIFO 16 项；每路软件接收队列 128 帧。
- FDCAN message RAM 偏移为 0、640、1280 **word**，每路占用 608 word；三路区域不重叠。
- USB 接收环形缓冲区 2048 字节，输出批次最多 512 字节；处理拆包、粘包、CRC 错误和超长报文后的恢复。
- 软件 RX 队列满时丢弃新帧并计数；硬件 RX 溢出计数是丢失事件次数，不能视作精确丢帧数。
- 每条发送请求最多等待 100 ms，随后请求取消；与取消竞争时若实际发送成功，仍报告成功。
- bus-off 后拒绝新增发送，每隔 1 秒尝试重新初始化该通道；在途请求会返回失败或取消。
- 关闭串口、USB reset 或重连会清理旧会话队列并取消在途发送，保留三路配置；尚未连接上位机时接收的帧不积存。
- MCU 时间戳是主循环取帧/处理发送结果的微秒时间，32 位约 71.6 分钟回卷。它不是 CAN SOF 硬件捕获时间。

## 验证

无需板卡即可检查协议和上位机分发，需要本机 C 编译器与 Python：

```powershell
.\tests\run.ps1
```

该检查运行实际固件协议 C 代码，验证 243 个 C/Python 互通向量、CRC 标准向量、拆包粘包、损坏/超长帧恢复、三路接收分发和超时不重发。

烧录后执行三路内部回环，工具会临时切换模式，结束后恢复原配置：

```powershell
python tools\dm_can.py --port COM7 loopback
python tools\dm_can.py --port COM7 loopback --fd
```

内部回环验证 MCU/USB/协议通路。外部 CAN 接线另用同速率 CAN 节点逐路互发，并检查 `stats` 中 `tx_success`、`tx_failed`、`rx_dropped` 和 `bus_off`。

本次已通过 ARM GNU 12.3.1 Release 编译及上述主机检查，未执行实板烧录和 CAN 总线测试。第三方代码出处见 [vendor/README.md](vendor/README.md)。
