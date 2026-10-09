# DM H723 USB CAN 协议 v1

一个 CDC 字节流承载三路 CAN。所有多字节整数为小端，通道号 1/2/3 对应 CAN1/2/3，通道 0 用于设备级命令。USB 传输边界不等于报文边界。

## 封装

原始报文经 COBS 编码后追加 `00` 分隔符。COBS 编码体不包含零字节。CRC 为 CRC16/CCITT-FALSE：poly `0x1021`，init `0xFFFF`，不反射，xorout `0`；输入 `123456789` 得到 `0x29B1`。

| 原始偏移 | 长度 | 字段 |
| --- | --- | --- |
| 0 | 1 | version，固定 1 |
| 1 | 1 | type |
| 2 | 1 | channel |
| 3 | 1 | flags |
| 4 | 2 | sequence |
| 6 | 2 | payload_length，0～80 |
| 8 | N | payload |
| 8+N | 2 | CRC，覆盖 version 至 payload 末尾 |

原始报文最多 90 字节，线上报文含分隔符最多 92 字节。长度、版本、CRC 无效的报文直接丢弃并增加统计；下一个 `00` 恢复报文边界。打开端口后置 DTR，并先发送一个 `00`，随后请求 INFO。

sequence 由主机选择，ACK 和 TX_RESULT 原样返回。主机应在等待某个发送结果期间保留该通道的 sequence，不重复使用。接收帧的 sequence 是固件递增计数，16 位回卷；该计数不取代丢帧统计。固件不缓存历史请求用于去重，不得盲目重发超时发送命令。

## flags

| 位 | 掩码 | 含义 |
| --- | --- | --- |
| 0 | 0x01 | 扩展 ID，29 位；否则标准 ID，11 位 |
| 1 | 0x02 | 经典 CAN 远程帧 RTR |
| 2 | 0x04 | CAN FD |
| 3 | 0x08 | FD 数据阶段 BRS |
| 4 | 0x10 | 接收方向 ESI passive |

TX 不接受 ESI 或未知标志。FD 不允许 RTR，BRS 仅能与 FD 同用。其余命令 flags 必须为 0。经典 CAN 长度为 0～8；FD 长度仅接受 `0,1,2,3,4,5,6,7,8,12,16,20,24,32,48,64`，主机自行显式填充数据到合法长度，固件不悄悄补齐。

## 请求与响应

| type | 命令 | 通道 | payload |
| --- | --- | --- | --- |
| 0x01 | INFO | 0 | 空 |
| 0x02 | CONFIG | 1～3 | nominal_bitrate u32、data_bitrate u32、mode u8 |
| 0x03 | STATE | 1～3 | enabled u8，0 关闭、1 打开 |
| 0x04 | TX | 1～3 | can_id u32、length u8、data[length] |
| 0x05 | STATS | 1～3 | 空 |
| 0x06 | PING | 0 | 任意 0～78 字节 |
| 0x80 | ACK | 原通道 | command_type u8、status u8、可选数据 |
| 0x81 | RX | 1～3 | can_id u32、timestamp_us u32、length u8、data[length] |
| 0x82 | TX_RESULT | 1～3 | status u8、timestamp_us u32 |

远程帧 TX/RX 仅携带 length，不附 data。TX length 与 ID 必须符合 flags 声明；FD 帧只能向启用了 FD 的通道发送。TX 成功 ACK 只表示接受入硬件 FIFO；总线成功发送后另返回 TX_RESULT，使用相同 sequence。未成功入队的 TX 只有错误 ACK，没有后续 TX_RESULT。

RX 的 sequence 独立于命令；ACK/RX/TX_RESULT 可穿插出现，上位机必须按 type、channel 和 sequence 分发。RX 的 flags 带实际接收帧属性。TX_RESULT 的 flags 为 0。

### CONFIG

先 STATE=0，再 CONFIG，最后 STATE=1。对打开通道直接 CONFIG 返回 BUSY。关闭通道会取消在途发送并清除该通道接收队列；重复打开正常运行的通道不取消在途帧。

| mode | 格式 | 工作方式 |
| --- | --- | --- |
| 0 | 经典 CAN | 正常 |
| 1 | CAN FD，可收发经典 CAN | 正常 |
| 2 | 经典 CAN | 只听 |
| 3 | CAN FD | 只听 |
| 4 | 经典 CAN | 内部回环 |
| 5 | CAN FD | 内部回环 |

支持仲裁速率 `50000,100000,125000,250000,500000,800000,1000000`；数据速率 `1000000,2000000,3000000,4000000,5000000,6000000,8000000`。经典 CAN 模式仍传入合法 data_bitrate（例如 2000000），该值仅保存，经典帧没有数据阶段变速。

### status

| 值 | 含义 |
| --- | --- |
| 0 | OK；TX_RESULT 中表示实际完成发送 |
| 1 | INVALID，类型、通道、flags、长度或参数无效 |
| 2 | BUSY，发送队列满、sequence 仍在途，或配置时通道没有关闭 |
| 3 | CLOSED，通道关闭或处于只听模式 |
| 4 | HAL_ERROR，底层操作失败 |
| 5 | TIMEOUT，100 ms 后请求取消，确认未成功发送 |
| 6 | CANCELED，关闭或恢复通道时清理在途请求 |
| 7 | BUS_OFF，拒绝新增发送 |

每路最多 16 个在途 TX，三路可同时提交。发生 bus-off 后固件每隔 1 秒恢复一次该通道；也可以关闭后再打开。重启通道时丢弃旧发送请求，避免恢复后突然发送积压命令。USB 会话结束后不发送旧会话的 ACK/TX_RESULT。

### INFO 数据

成功 ACK 的 command/status 后：firmware_major u8、minor u8、patch u8、channel_count u8（3）、max_data_length u8（64）、tx_slots u8（每路 16）、capabilities u16、board_name ASCII 剩余字节（`DM-H723`，无末尾 NUL）。

capabilities bit0=经典 CAN，bit1=CAN FD，bit2=只听，bit3=内部回环。

### STATS 数据

成功 ACK 的 command/status 后按顺序为下列 18 个 u32，共 72 字节：

1. nominal_bitrate
2. data_bitrate
3. mode
4. enabled
5. bus_off，当前硬件状态
6. tx_error_counter
7. rx_error_counter
8. rx_frames，从硬件 FIFO 读出的累计帧数
9. rx_dropped，软件队列满、会话未打开或清理接收队列时丢弃的累计帧数
10. hardware_rx_loss_events，硬件 FIFO 丢失标志被观察到的次数
11. tx_queued，接受的累计发送请求
12. tx_success，确认总线发送成功的累计请求
13. tx_failed，超时或清理在途请求的累计次数
14. bus_off_events
15. tx_event_loss_events
16. protocol_errors，整个设备共享
17. control_dropped，整个设备共享
18. usb_rx_overflow，整个设备共享，正常背压下应为零

计数随复位清零，u32 自然回卷。硬件丢失事件可以合并多次丢帧，不是精确丢帧数。只听模式不会主动产生 CAN ACK。

PING 成功 ACK 的后续数据为请求 payload 原样回显。

## 时间与会话

timestamp_us 是固件运行以来的 32 位微秒时间，約 71.6 分钟回卷，代表主循环从 FIFO 取帧或观察发送完成的时间。接收帧真正到达总线的时间早于软件取样；跨 USB 主机的到达时间也不能直接替代该时间。

串口打开/关闭由 DTR 控制，USB reset/重连也会结束旧会话。清理软件和硬件队列时保留通道波特率、模式和开关状态。一次 USB 会话由一个主机程序管理；主机重开后重新读取 INFO/STATS，再按需求配置三路。
