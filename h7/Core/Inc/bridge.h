#ifndef BRIDGE_H
#define BRIDGE_H
#include <stdint.h>
void bridge_init(void);
void bridge_poll(void);
/* USB 回调只发布数据，避免在中断里执行 CAN 配置和协议命令。 */
void bridge_usb_session(uint8_t active);
void bridge_usb_rx(const uint8_t *data, uint32_t length);
uint8_t bridge_usb_rx_space(void);
#endif
