#ifndef BOARD_H
#define BOARD_H
#include "main.h"
#define CAN_CHANNELS 3U
#define CAN_KERNEL_HZ 120000000UL
#define CAN_TX_SLOTS 16U
extern FDCAN_HandleTypeDef board_can[CAN_CHANNELS];
void board_init(void);
uint32_t board_micros(void);
uint8_t board_can_configure(unsigned channel, uint32_t nominal, uint32_t data_rate, uint8_t mode);
uint8_t board_can_state(unsigned channel, uint8_t enabled);
#endif
