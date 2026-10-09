#include "motor_can.h"
#include "board.h"

static uint32_t requested_ms[MOTOR_COUNT];
static uint8_t requested[MOTOR_COUNT];

uint8_t motor_can_ready(void)
{
    FDCAN_HandleTypeDef *h=&board_can[MOTOR_CAN_CHANNEL-1U];
    uint32_t divider=h->Init.NominalPrescaler*(1U+h->Init.NominalTimeSeg1+h->Init.NominalTimeSeg2);
    return h->State==HAL_FDCAN_STATE_BUSY && h->Init.Mode==FDCAN_MODE_NORMAL &&
           h->Init.FrameFormat==FDCAN_FRAME_CLASSIC &&
           divider==CAN_KERNEL_HZ/1000000U && !(h->Instance->PSR & FDCAN_PSR_BO);
}

void motor_can_cancel(unsigned motor)
{
    if (motor>=MOTOR_COUNT) return;
    FDCAN_HandleTypeDef *h=&board_can[MOTOR_CAN_CHANNEL-1U];
    if (h->State==HAL_FDCAN_STATE_BUSY) HAL_FDCAN_AbortTxRequest(h,1UL<<motor);
    requested[motor]=0;
}

uint8_t motor_can_send(unsigned motor,uint16_t id,const uint8_t data[8],uint8_t urgent)
{
    if (motor>=MOTOR_COUNT || !motor_can_ready()) return MOTOR_SEND_ERROR;
    FDCAN_HandleTypeDef *h=&board_can[MOTOR_CAN_CHANNEL-1U];
    uint32_t mask=1UL<<motor,now=HAL_GetTick();
    if (HAL_FDCAN_IsTxBufferMessagePending(h,mask)) {
        /* 独立 Tx buffer 只保存最新控制量，不能在透传 FIFO 中堆积旧电流命令。 */
        if (urgent || !requested[motor] || now-requested_ms[motor]>=2U)
            HAL_FDCAN_AbortTxRequest(h,mask);
        return MOTOR_SEND_BUSY;
    }
    FDCAN_TxHeaderTypeDef header={0};
    header.Identifier=id;
    header.IdType=FDCAN_STANDARD_ID;
    header.TxFrameType=FDCAN_DATA_FRAME;
    header.DataLength=FDCAN_DLC_BYTES_8;
    header.ErrorStateIndicator=FDCAN_ESI_ACTIVE;
    header.BitRateSwitch=FDCAN_BRS_OFF;
    header.FDFormat=FDCAN_CLASSIC_CAN;
    header.TxEventFifoControl=FDCAN_NO_TX_EVENTS;
    /* HAL 的 BufferIndex 参数是位掩码，写入和请求必须指向同一个硬件槽位。 */
    if (HAL_FDCAN_AddMessageToTxBuffer(h,&header,data,mask)!=HAL_OK ||
        HAL_FDCAN_EnableTxBufferRequest(h,mask)!=HAL_OK) return MOTOR_SEND_ERROR;
    requested_ms[motor]=now;
    requested[motor]=1;
    return MOTOR_SEND_OK;
}
