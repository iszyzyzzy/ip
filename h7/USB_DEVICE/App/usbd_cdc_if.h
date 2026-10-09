#ifndef USBD_CDC_IF_H
#define USBD_CDC_IF_H
#include "usbd_cdc.h"
extern USBD_CDC_ItfTypeDef USBD_Interface_fops_HS;
uint8_t CDC_Transmit_HS(uint8_t *data,uint16_t length);
uint8_t cdc_tx_ready(void);
void cdc_poll(void);
#endif
