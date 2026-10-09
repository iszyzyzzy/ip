# 第三方代码来源

- STM32CubeH7 **v1.12.1**：HAL、CMSIS 头文件、STM32H723 GCC 启动文件、system 初始化文件和 USB Device CDC 库，来自本机 ST 官方 STM32Cube 包。保留原文件版权声明，完整包许可证见 `Package_license.md`；HAL 和 USB 组件亦各自保留 `LICENSE.txt`。
- 板卡引脚、时钟和 USB 适配文件依据 [dmBots/DM-MC02](https://github.com/dmBots/DM-MC02)，commit `d65a3688981405dc205f628f1a026bde8cf8303e`。
- FDCAN 配置依据 `examples/CtrBoard-H7_ALL/Core/Src/fdcan.c`；HSE/USB 配置依据 `examples/CtrBoard-H7_USB/Core/Src/main.c` 和 `CtrBoard-H7_ALL.ioc`。
- `USB_DEVICE/App/usb_device.*`、`usbd_desc.*` 和 `USB_DEVICE/Target/usbd_conf.*` 由厂家 ST 生成文件适配；调整 CDC 描述、接口数量、LPM、USB 中断优先级和通知端点 FIFO。
- 应用层 `Core/Src/bridge.c`、`protocol.c`、`board.c`、上位机和项目脚本为本工程实现。
