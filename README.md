# 机器人控制工程

[robotd](robotd/README.md) 提供统一 gRPC 控制、SQLite 权威配置与动作存储，连接 H7、双臂总线舵机和眼屏。

- [H7 固件与上位机协议](h7/README.md)：三路 CAN、GM6020/DM4310 位置闭环。
- [眼屏渲染](eyes/README.md)：双眼渲染及本地 gRPC。
- [底盘协议资料](agv.md)：RANGER AIR。

## H7

固件、Python 上位机接口和使用说明位于 [h7](h7/README.md)。

目标板：达妙 DM-MC02 / DM-MC-Board02，STM32H723VGT6。
