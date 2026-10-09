#ifndef MOTOR_CAN_H
#define MOTOR_CAN_H
#include "motor.h"
uint8_t motor_can_send(unsigned motor,uint16_t id,const uint8_t data[8],uint8_t urgent);
uint8_t motor_can_ready(void);
void motor_can_cancel(unsigned motor);
#endif
