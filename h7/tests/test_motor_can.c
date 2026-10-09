#include "board.h"
#include "motor_can.h"
#include <assert.h>
#include <stdio.h>

FDCAN_HandleTypeDef board_can[3];
static test_registers_t registers;
static uint32_t tick,pending,last_write,last_request,last_cancel,writes;
uint32_t HAL_GetTick(void) { return tick; }
uint32_t HAL_FDCAN_IsTxBufferMessagePending(FDCAN_HandleTypeDef *h,uint32_t mask) { (void)h; return pending&mask; }
uint32_t HAL_FDCAN_AbortTxRequest(FDCAN_HandleTypeDef *h,uint32_t mask) { (void)h; last_cancel=mask; return HAL_OK; }
uint32_t HAL_FDCAN_AddMessageToTxBuffer(FDCAN_HandleTypeDef *h,const FDCAN_TxHeaderTypeDef *header,const uint8_t *data,uint32_t mask)
{
    assert(h==&board_can[MOTOR_CAN_CHANNEL-1]);
    /* 与库内 POSITION_VAL(BufferIndex) 一致，这里检查单个位而非从零开始的编号。 */
    assert(mask==1 || mask==2); assert(!(pending&mask));
    assert(header->DataLength==FDCAN_DLC_BYTES_8 && header->TxEventFifoControl==FDCAN_NO_TX_EVENTS);
    (void)data; last_write=mask; ++writes;
    return HAL_OK;
}
uint32_t HAL_FDCAN_EnableTxBufferRequest(FDCAN_HandleTypeDef *h,uint32_t mask)
{
    (void)h; assert(mask==last_write); last_request=mask; pending|=mask; return HAL_OK;
}
int main(void)
{
    FDCAN_HandleTypeDef *h=&board_can[MOTOR_CAN_CHANNEL-1];
    h->State=HAL_FDCAN_STATE_BUSY; h->Instance=&registers;
    h->Init=(test_init_t){6,15,4,FDCAN_MODE_NORMAL,FDCAN_FRAME_CLASSIC};
    uint8_t data[8]={0}; assert(motor_can_ready());
    assert(motor_can_send(0,0x1fe,data,0)==MOTOR_SEND_OK && last_request==1);
    assert(motor_can_send(1,1,data,0)==MOTOR_SEND_OK && last_request==2);
    assert(motor_can_send(0,0x1fe,data,0)==MOTOR_SEND_BUSY && last_cancel==0 && writes==2);
    tick=2;
    assert(motor_can_send(0,0x1fe,data,0)==MOTOR_SEND_BUSY && last_cancel==1 && writes==2);
    motor_can_cancel(1); assert(last_cancel==2);
    pending=0; assert(motor_can_send(1,1,data,0)==MOTOR_SEND_OK && last_write==2);
    registers.PSR=FDCAN_PSR_BO; assert(!motor_can_ready());
    registers.PSR=0; h->Init.FrameFormat=1; assert(!motor_can_ready());
    h->Init.FrameFormat=0; h->Init.NominalPrescaler=12; assert(!motor_can_ready());
    puts("电机 CAN 适配检查通过：独立槽位掩码、禁止覆盖在途帧及总线模式检查");
    return 0;
}
