#include "board.h"

FDCAN_HandleTypeDef board_can[CAN_CHANNELS];

static void clock_init(void)
{
    RCC_OscInitTypeDef osc={0};
    RCC_ClkInitTypeDef clk={0};
    HAL_PWREx_ConfigSupply(PWR_LDO_SUPPLY);
    __HAL_PWR_VOLTAGESCALING_CONFIG(PWR_REGULATOR_VOLTAGE_SCALE0);
    while (!__HAL_PWR_GET_FLAG(PWR_FLAG_VOSRDY)) {}
    osc.OscillatorType=RCC_OSCILLATORTYPE_HSE|RCC_OSCILLATORTYPE_HSI48;
    osc.HSEState=RCC_HSE_ON;
    osc.HSI48State=RCC_HSI48_ON;
    osc.PLL.PLLState=RCC_PLL_ON;
    osc.PLL.PLLSource=RCC_PLLSOURCE_HSE;
    osc.PLL.PLLM=2;
    osc.PLL.PLLN=40;
    osc.PLL.PLLP=1;
    osc.PLL.PLLQ=4;
    osc.PLL.PLLR=2;
    osc.PLL.PLLRGE=RCC_PLL1VCIRANGE_3;
    osc.PLL.PLLVCOSEL=RCC_PLL1VCOWIDE;
    if (HAL_RCC_OscConfig(&osc)!=HAL_OK) Error_Handler();
    clk.ClockType=RCC_CLOCKTYPE_SYSCLK|RCC_CLOCKTYPE_HCLK|RCC_CLOCKTYPE_PCLK1|
                  RCC_CLOCKTYPE_PCLK2|RCC_CLOCKTYPE_D1PCLK1|RCC_CLOCKTYPE_D3PCLK1;
    clk.SYSCLKSource=RCC_SYSCLKSOURCE_PLLCLK;
    clk.SYSCLKDivider=RCC_SYSCLK_DIV1;
    clk.AHBCLKDivider=RCC_HCLK_DIV2;
    clk.APB1CLKDivider=RCC_APB1_DIV2;
    clk.APB2CLKDivider=RCC_APB2_DIV2;
    clk.APB3CLKDivider=RCC_APB3_DIV2;
    clk.APB4CLKDivider=RCC_APB4_DIV2;
    if (HAL_RCC_ClockConfig(&clk,FLASH_LATENCY_3)!=HAL_OK) Error_Handler();

    /* USB SOF 校准 HSI48，避免内置振荡器误差导致枚举或持续传输不稳定。 */
    __HAL_RCC_CRS_CLK_ENABLE();
    RCC_CRSInitTypeDef crs={0};
    crs.Source=RCC_CRS_SYNC_SOURCE_USB1;
    crs.Polarity=RCC_CRS_SYNC_POLARITY_RISING;
    crs.Prescaler=RCC_CRS_SYNC_DIV1;
    crs.ReloadValue=__HAL_RCC_CRS_RELOADVALUE_CALCULATE(48000000,1000);
    crs.ErrorLimitValue=34;
    crs.HSI48CalibrationValue=32;
    HAL_RCCEx_CRSConfig(&crs);
}

void board_init(void)
{
    clock_init();
    SCB_EnableICache();
    /* USB 不使用 DMA；所有缓冲区由 CPU 访问，可以直接启用 D-cache。 */
    SCB_EnableDCache();
    __HAL_RCC_GPIOB_CLK_ENABLE();
    __HAL_RCC_GPIOD_CLK_ENABLE();
    __HAL_RCC_FDCAN_CLK_ENABLE();
    RCC_PeriphCLKInitTypeDef periph={0};
    periph.PeriphClockSelection=RCC_PERIPHCLK_FDCAN;
    periph.FdcanClockSelection=RCC_FDCANCLKSOURCE_PLL;
    if (HAL_RCCEx_PeriphCLKConfig(&periph)!=HAL_OK) Error_Handler();
    GPIO_InitTypeDef gpio={0};
    gpio.Mode=GPIO_MODE_AF_PP;
    gpio.Pull=GPIO_NOPULL;
    gpio.Speed=GPIO_SPEED_FREQ_VERY_HIGH;
    gpio.Pin=GPIO_PIN_0|GPIO_PIN_1;
    gpio.Alternate=GPIO_AF9_FDCAN1;
    HAL_GPIO_Init(GPIOD,&gpio);
    gpio.Pin=GPIO_PIN_5|GPIO_PIN_6;
    gpio.Alternate=GPIO_AF9_FDCAN2;
    HAL_GPIO_Init(GPIOB,&gpio);
    gpio.Pin=GPIO_PIN_12|GPIO_PIN_13;
    gpio.Alternate=GPIO_AF5_FDCAN3;
    HAL_GPIO_Init(GPIOD,&gpio);
    board_can[0].Instance=FDCAN1;
    board_can[1].Instance=FDCAN2;
    board_can[2].Instance=FDCAN3;
}

uint32_t board_micros(void)
{
    /* SysTick 是 1 ms 周期；重读 tick 排除计数器在取样中途回卷。 */
    uint32_t a,b,cycles;
    do {
        a=HAL_GetTick();
        cycles=SysTick->VAL;
        b=HAL_GetTick();
    } while (a!=b || (SCB->ICSR & SCB_ICSR_PENDSTSET_Msk));
    return a*1000U+(SysTick->LOAD-cycles)/(SystemCoreClock/1000000U);
}

static uint8_t nominal_timing(FDCAN_InitTypeDef *init,uint32_t rate)
{
    switch (rate) {
    case 50000: case 100000: case 125000: case 250000: case 500000: case 1000000:
        init->NominalPrescaler=1000000U/rate;
        init->NominalTimeSeg1=95;
        init->NominalTimeSeg2=24;
        init->NominalSyncJumpWidth=24;
        return 1;
    case 800000:
        init->NominalPrescaler=1;
        init->NominalTimeSeg1=119;
        init->NominalTimeSeg2=30;
        init->NominalSyncJumpWidth=30;
        return 1;
    default: return 0;
    }
}

static uint8_t data_timing(FDCAN_InitTypeDef *init,uint32_t rate)
{
    uint32_t tq=20, prescaler;
    switch (rate) {
    case 1000000: prescaler=6; break;
    case 2000000: prescaler=3; break;
    case 3000000: prescaler=2; break;
    case 4000000: prescaler=2; tq=15; break;
    case 5000000: prescaler=1; tq=24; break;
    case 6000000: prescaler=1; break;
    case 8000000: prescaler=1; tq=15; break;
    default: return 0;
    }
    init->DataPrescaler=prescaler;
    init->DataTimeSeg2=(tq+3U)/4U;
    init->DataTimeSeg1=tq-1U-init->DataTimeSeg2;
    init->DataSyncJumpWidth=init->DataTimeSeg2;
    return 1;
}

uint8_t board_can_configure(unsigned ch,uint32_t nominal,uint32_t data_rate,uint8_t mode)
{
    if (ch>=CAN_CHANNELS || mode>5) return 1;
    FDCAN_HandleTypeDef *h=&board_can[ch];
    FDCAN_InitTypeDef init={0};
    if (!nominal_timing(&init,nominal) || !data_timing(&init,data_rate)) return 1;
    init.FrameFormat=(mode&1U) ? FDCAN_FRAME_FD_BRS : FDCAN_FRAME_CLASSIC;
    init.Mode=mode<2 ? FDCAN_MODE_NORMAL : (mode<4 ? FDCAN_MODE_BUS_MONITORING : FDCAN_MODE_INTERNAL_LOOPBACK);
    init.AutoRetransmission=ENABLE;
    init.TransmitPause=DISABLE;
    init.ProtocolException=ENABLE;
    /* 两个专用电机 Tx buffer 加上透传 FIFO，每路 644 word，三路独立分区。 */
    init.MessageRAMOffset=ch*704U;
    init.RxFifo0ElmtsNbr=16;
    init.RxFifo0ElmtSize=FDCAN_DATA_BYTES_64;
    init.RxFifo1ElmtSize=FDCAN_DATA_BYTES_64;
    init.RxBufferSize=FDCAN_DATA_BYTES_64;
    init.TxEventsNbr=16;
    init.TxBuffersNbr=2;
    init.TxFifoQueueElmtsNbr=CAN_TX_SLOTS;
    init.TxFifoQueueMode=FDCAN_TX_FIFO_OPERATION;
    init.TxElmtSize=FDCAN_DATA_BYTES_64;
    if (h->State==HAL_FDCAN_STATE_BUSY && HAL_FDCAN_Stop(h)!=HAL_OK) return 4;
    h->Init=init;
    if (HAL_FDCAN_Init(h)!=HAL_OK) return 4;
    if (HAL_FDCAN_ConfigGlobalFilter(h,FDCAN_ACCEPT_IN_RX_FIFO0,FDCAN_ACCEPT_IN_RX_FIFO0,
                                   FDCAN_FILTER_REMOTE,FDCAN_FILTER_REMOTE)!=HAL_OK) return 4;
    if (HAL_FDCAN_ConfigRxFifoOverwrite(h,FDCAN_RX_FIFO0,FDCAN_RX_FIFO_BLOCKING)!=HAL_OK) return 4;
    if (HAL_FDCAN_DisableTxDelayCompensation(h)!=HAL_OK) return 4;
    if ((mode&1U) && mode<2 && data_rate>1000000U) {
        uint32_t offset=init.DataPrescaler*init.DataTimeSeg1;
        if (HAL_FDCAN_ConfigTxDelayCompensation(h,offset,0)!=HAL_OK ||
            HAL_FDCAN_EnableTxDelayCompensation(h)!=HAL_OK) return 4;
    }
    return 0;
}

uint8_t board_can_state(unsigned ch,uint8_t enabled)
{
    if (ch>=CAN_CHANNELS) return 1;
    FDCAN_HandleTypeDef *h=&board_can[ch];
    if (enabled) {
        if (h->State==HAL_FDCAN_STATE_BUSY) return 0;
        return HAL_FDCAN_Start(h)==HAL_OK ? 0 : 4;
    }
    if (h->State!=HAL_FDCAN_STATE_BUSY) return 0;
    return HAL_FDCAN_Stop(h)==HAL_OK ? 0 : 4;
}
