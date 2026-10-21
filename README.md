# 机器人控制工程

[robotd](robotd/README.md) 是 Rust 实现的统一 gRPC 控制中枢，使用 SQLite 存储权威配置与动作，连接 H7、双臂总线舵机、眼屏和 TAS5756M I²S 喇叭；音频使用低延迟有界缓冲。

[robot-remote](robot_remote/README.md) 独立运行 HTTP/WebSocket 遥控服务，为网页提供状态推送、控制会话、运动目标和动作接口，通过 gRPC 连接 robotd。

- [H7 固件与上位机协议](h7/README.md)：三路 CAN、GM6020/DM4310 位置闭环。
- [眼屏渲染](eyes/README.md)：双眼渲染及本地 gRPC。
- [底盘协议资料](agv.md)：RANGER AIR。

## H7

固件、Python 上位机接口和使用说明位于 [h7](h7/README.md)。

目标板：达妙 DM-MC02 / DM-MC-Board02，STM32H723VGT6。
