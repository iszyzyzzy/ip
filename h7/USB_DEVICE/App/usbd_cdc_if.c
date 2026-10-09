#include "usbd_cdc_if.h"
#include "usb_device.h"
#include "bridge.h"
#include <string.h>

static uint8_t rx_buffer[64];
static uint8_t tx_dummy;
static uint8_t line_coding[7]={0x00,0xc2,0x01,0x00,0,0,8};
static volatile uint8_t rx_paused;

static int8_t cdc_init(void)
{
    rx_paused=0;
    bridge_usb_session(0);
    USBD_CDC_SetTxBuffer(&hUsbDeviceHS,&tx_dummy,0);
    USBD_CDC_SetRxBuffer(&hUsbDeviceHS,rx_buffer);
    return USBD_OK;
}
static int8_t cdc_deinit(void)
{
    rx_paused=0;
    bridge_usb_session(0);
    return USBD_OK;
}
static int8_t cdc_control(uint8_t cmd,uint8_t *buf,uint16_t length)
{
    if (cmd==CDC_SET_LINE_CODING && length==7) memcpy(line_coding,buf,7);
    else if (cmd==CDC_GET_LINE_CODING && length==7) memcpy(buf,line_coding,7);
    else if (cmd==CDC_SET_CONTROL_LINE_STATE) {
        USBD_SetupReqTypedef *request=(USBD_SetupReqTypedef *)buf;
        bridge_usb_session((uint8_t)(request->wValue & 1U));
    }
    return USBD_OK;
}
static int8_t cdc_receive(uint8_t *buf,uint32_t *length)
{
    bridge_usb_rx(buf,*length);
    /* 暂停重挂 OUT 端点会让 USB 返回 NAK，从源头避免接收环形缓冲区溢出。 */
    if (bridge_usb_rx_space()) USBD_CDC_ReceivePacket(&hUsbDeviceHS);
    else rx_paused=1;
    return USBD_OK;
}
static int8_t cdc_complete(uint8_t *buf,uint32_t *length,uint8_t ep)
{
    (void)buf; (void)length; (void)ep;
    return USBD_OK;
}
USBD_CDC_ItfTypeDef USBD_Interface_fops_HS={cdc_init,cdc_deinit,cdc_control,cdc_receive,cdc_complete};

void cdc_poll(void)
{
    uint32_t irq=__get_PRIMASK();
    __disable_irq();
    if (rx_paused && hUsbDeviceHS.dev_state==USBD_STATE_CONFIGURED &&
        hUsbDeviceHS.pClassData && bridge_usb_rx_space()) {
        rx_paused=0;
        USBD_CDC_ReceivePacket(&hUsbDeviceHS);
    }
    __set_PRIMASK(irq);
}
uint8_t cdc_tx_ready(void)
{
    USBD_CDC_HandleTypeDef *cdc=hUsbDeviceHS.pClassData;
    return hUsbDeviceHS.dev_state==USBD_STATE_CONFIGURED && cdc && cdc->TxState==0;
}
uint8_t CDC_Transmit_HS(uint8_t *data,uint16_t length)
{
    /* Reset/拔插会在中断里释放 class 状态，因此检查和提交必须处于同一临界区。 */
    uint32_t irq=__get_PRIMASK();
    __disable_irq();
    uint8_t status=USBD_BUSY;
    if (cdc_tx_ready()) {
        USBD_CDC_SetTxBuffer(&hUsbDeviceHS,data,length);
        status=USBD_CDC_TransmitPacket(&hUsbDeviceHS);
    }
    __set_PRIMASK(irq);
    return status;
}
