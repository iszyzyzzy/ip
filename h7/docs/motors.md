# CDC 双电机位置控制

GM6020 和 DM4310 接在同一条 CAN1 总线上，1 Mbps 经典 CAN、正常模式。两套位置环和速度环在 H723 上独立运行；CDC 直接发送目标即可启动控制，省略的电机维持原目标和状态。默认配置在 `Core/Inc/motor_config.h`：

| 项目 | 默认值 |
| --- | --- |
| 共享通道 | CAN1，PD0 RX / PD1 TX |
| GM6020 拨码 ID | 1，反馈 ID 0x205，电流指令 ID 0x1FE |
| DM4310 motor CAN ID | 0x01，MIT 模式 |
| DM4310 master/反馈 ID | 0x11 |
| 位置参考 | 每个电机首次反馈的位置为自己的 0 rad |
| 控制周期 | 主循环按 1 ms 调度；停顿超过 10 ms 时停止 |
| 反馈超时 | 100 ms，DM 首次使能等待最多 300 ms |
| 温度停止阈值 | 80 ℃ |

两个电机各自供电，两电机与控制板 CAN_H、CAN_L、地相连，CAN 总线两端各 120 Ω。目标和反馈均为电机轴位置，没有额外传动比换算。

## 电机设置

GM6020 在 RoboMaster Assistant 中启用**内置电流环**，拨码设 ID1。[大疆官方 v1.4 手册](https://rm-static.djicdn.com/tem/17348/RoboMaster%20GM6020%E7%9B%B4%E6%B5%81%E6%97%A0%E5%88%B7%E7%94%B5%E6%9C%BA%E4%BD%BF%E7%94%A8%E8%AF%B4%E6%98%8E20231013.pdf)要求电机固件至少 1.0.11.2、调参软件至少 2.7；电流环指令使用 0x1FE / 0x2FE，±16384 对应 ±3 A。默认 ID1 占指令前两个字节，其余槽位发送零电流。

DM4310 在达妙调试软件中设 MIT 模式、motor ID=0x01、master ID=0x11、CAN 1 Mbps。位置、速度和力矩映射采用[达妙官方 STM32 示例](https://github.com/dmBots/motor-control-routine/tree/ae6c1031056908d3d8059f003bd015a9d6b21894/stm32%E4%BE%8B%E7%A8%8B)的 DM4310 参数：位置 ±12.5 rad、速度 ±30 rad/s、力矩 ±10 Nm。电机固件的 PMAX/VMAX/TMAX 必须与这些值一致。MIT 指令的 Kp、Kd 为零，由 H723 的位置 P → 速度 PI 输出力矩；避免两套位置环同时作用。

使用其他 CAN 通道/ID 时修改 `motor_config.h` 并重新编译，两电机仍共用其中的一个通道。CAN2、CAN3 可以独立继续使用桥接功能。电机所在通道保持经典 CAN 1 Mbps，其他模式下位置命令会被拒绝。

## 直接发位置

Python API 单位为 **rad**，线上的位置字段也是 rad。命令行 `--gm/--dm` 为方便操作使用**度**，工具负责转换。

```powershell
# 同时设置 GM6020=30°、DM4310=-20°，维持位置直到 Ctrl+C。
python tools\dm_can.py --port COM7 motor-position --gm 30 --dm -20

# 仅控制 DM4310=15°，运行 5 秒后停止。
python tools\dm_can.py --port COM7 motor-position --dm 15 --seconds 5

python tools\dm_can.py --port COM7 motor-status
python tools\dm_can.py --port COM7 motor-stop
python tools\dm_can.py --port COM7 motor-zero --mask 3
```

命令行位置工具持续读取状态；停止、Ctrl+C 或运行时间结束时发送 STOP。关闭串口或 USB 断开会停止控制，因此自己的上位机应保持同一个 CDC 会话打开。运行期间查询、修改目标和单独停止应在同一程序里完成：

```python
import math
import sys
import time
sys.path.insert(0, r'D:\code\ip\h7\tools')
from dm_can import Bridge

with Bridge('COM7') as board:
    # GM6020 上电后需要先收到反馈，查询也可确认驱动配置是否正确。
    print(board.motor_status())
    try:
        board.motor_position(gm=math.radians(30), dm=math.radians(-20))
        time.sleep(1)
        board.motor_position(dm=math.radians(10))  # GM6020 继续保持 30°。
        time.sleep(1)
        board.motor_stop(mask=1)                  # DM4310 继续保持 10°。
        print(board.motor_status())
        time.sleep(1)
    finally:
        board.motor_stop()
```

打开/关闭 CDC 会话时固件会清理在途 CAN 帧，正在运行的电机停止。独立启动一个 `motor-status` 命令会开启新会话，不能作为另一个占用串口程序的旁路监控。

位置是相对于软件零点的连续角度，例如 2π 为正向一圈。跨圈通过反馈展开；失联超过 100 ms 后圈数可能丢失，恢复反馈后先 STOP、ZERO，再发新目标。ZERO 只改变 H723 的参考，不发送达妙永久保存零点指令。正常收到目标 ACK 不代表已经到位，应比较状态里的实际位置、目标和故障状态。

## 控制参数

| 参数 | GM6020 | DM4310 |
| --- | --- | --- |
| position_kp | 8 | 6 |
| velocity_kp | 0.12 | 0.25 |
| velocity_ki | 0.30 | 0.50 |
| max_velocity | 3 rad/s | 3 rad/s |
| max_output | 0.5 A | 0.5 Nm |
| max_position | ±12.5 rad | ±12.5 rad |

这些值作为初始调试参数，实际负载需要调参。位置 P 环限制目标速度，速度 PI 环限制电流/力矩并抑制饱和积分；没有运动轨迹规划。max_position 只限制目标相对零点的绝对值。

```powershell
# 电机停止后设置 GM6020 参数，修改仅保存在 RAM。
python tools\dm_can.py --port COM7 motor-tune --motor 1 --position-kp 8 --velocity-kp 0.12 --velocity-ki 0.3 --max-velocity 2 --max-output 0.4 --max-position 6.28
```

首次调试可在无负载条件下先发几度目标，读取位置/输出方向，再逐步调增益和限幅。固件会在 USB 会话结束、反馈失效、驱动报错、过温、总线不可用、持续发送阻塞或调度超时时停止；恢复条件后重新发送位置才重新启动。

固件已通过交叉编译和运行实际控制代码的本机检查；尚未进行这两个电机的实机闭环调参。
