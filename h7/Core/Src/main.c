#include "board.h"
#include "bridge.h"
#include "usb_device.h"
extern PCD_HandleTypeDef hpcd_USB_OTG_HS;

int main(void)
{
    HAL_Init();
    board_init();
    bridge_init();
    MX_USB_DEVICE_Init();
    for (;;) bridge_poll();
}

void HAL_MspInit(void)
{
    __HAL_RCC_SYSCFG_CLK_ENABLE();
    HAL_NVIC_SetPriorityGrouping(NVIC_PRIORITYGROUP_4);
}
void SysTick_Handler(void) { HAL_IncTick(); }
/* 自定义启动文件负责 RAM 初始化，不链接宿主式 CRT 的退出和标准输入输出路径。 */
void _init(void) {}
void _fini(void) {}
void OTG_HS_IRQHandler(void) { HAL_PCD_IRQHandler(&hpcd_USB_OTG_HS); }
void Error_Handler(void)
{
    __disable_irq();
    for (;;) __NOP();
}
void HardFault_Handler(void) { Error_Handler(); }
void MemManage_Handler(void) { Error_Handler(); }
void BusFault_Handler(void) { Error_Handler(); }
void UsageFault_Handler(void) { Error_Handler(); }
