#ifndef TEST_BOARD_H
#define TEST_BOARD_H
#include <stdint.h>
#define CAN_KERNEL_HZ 120000000U
#define HAL_FDCAN_STATE_BUSY 2U
#define FDCAN_MODE_NORMAL 0U
#define FDCAN_FRAME_CLASSIC 0U
#define FDCAN_PSR_BO 128U
#define FDCAN_STANDARD_ID 0U
#define FDCAN_DATA_FRAME 0U
#define FDCAN_DLC_BYTES_8 8U
#define FDCAN_ESI_ACTIVE 0U
#define FDCAN_BRS_OFF 0U
#define FDCAN_CLASSIC_CAN 0U
#define FDCAN_NO_TX_EVENTS 0U
#define HAL_OK 0U
typedef struct { uint32_t PSR; } test_registers_t;
typedef struct {
    uint32_t NominalPrescaler,NominalTimeSeg1,NominalTimeSeg2,Mode,FrameFormat;
} test_init_t;
typedef struct { uint32_t State; test_init_t Init; test_registers_t *Instance; } FDCAN_HandleTypeDef;
typedef struct {
    uint32_t Identifier,IdType,TxFrameType,DataLength,ErrorStateIndicator;
    uint32_t BitRateSwitch,FDFormat,TxEventFifoControl;
} FDCAN_TxHeaderTypeDef;
extern FDCAN_HandleTypeDef board_can[3];
uint32_t HAL_GetTick(void);
uint32_t HAL_FDCAN_IsTxBufferMessagePending(FDCAN_HandleTypeDef *,uint32_t);
uint32_t HAL_FDCAN_AbortTxRequest(FDCAN_HandleTypeDef *,uint32_t);
uint32_t HAL_FDCAN_AddMessageToTxBuffer(FDCAN_HandleTypeDef *,const FDCAN_TxHeaderTypeDef *,const uint8_t *,uint32_t);
uint32_t HAL_FDCAN_EnableTxBufferRequest(FDCAN_HandleTypeDef *,uint32_t);
#endif
