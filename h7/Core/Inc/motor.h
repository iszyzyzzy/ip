#ifndef MOTOR_H
#define MOTOR_H
#include "protocol.h"
#include "motor_config.h"
enum { MOTOR_GM=0, MOTOR_DM=1, MOTOR_COUNT=2 };
enum { MOTOR_STOPPED=0, MOTOR_STARTING=1, MOTOR_RUNNING=2, MOTOR_FAULT=3 };
enum { MOTOR_FAULT_NONE=0, MOTOR_FAULT_FEEDBACK=1, MOTOR_FAULT_DRIVER=2,
       MOTOR_FAULT_BUS=3, MOTOR_FAULT_TX=4, MOTOR_FAULT_HOST=5,
       MOTOR_FAULT_TIMING=6, MOTOR_FAULT_REFERENCE=7, MOTOR_FAULT_TEMPERATURE=8 };
enum { MOTOR_SEND_OK=0, MOTOR_SEND_BUSY=1, MOTOR_SEND_ERROR=2 };
typedef struct {
    float position_kp,velocity_kp,velocity_ki,max_velocity,max_output,max_position;
} motor_tuning_t;
typedef struct {
    uint8_t state,fault,temperature,driver_state,have_feedback,reference_valid,claimed;
    uint32_t feedback_ms,started_ms,tx_failures;
    float position,velocity,effort,target,output,integral;
    float previous_raw,origin,absolute;
    motor_tuning_t tuning;
} motor_state_t;
typedef uint8_t (*motor_send_fn)(unsigned motor,uint16_t id,const uint8_t data[8],uint8_t urgent);
typedef uint8_t (*motor_bus_ready_fn)(void);
typedef void (*motor_cancel_fn)(unsigned motor);
void motor_init(motor_send_fn send,motor_bus_ready_fn ready,motor_cancel_fn cancel);
void motor_poll(uint32_t now_ms,uint8_t session_active);
void motor_receive(unsigned channel,uint32_t id,uint8_t flags,const uint8_t *data,uint8_t length,uint32_t now_ms);
uint8_t motor_command(const proto_packet_t *request,proto_packet_t *response,uint32_t now_ms);
void motor_stop(uint8_t mask,uint8_t fault);
void motor_bus_reset(unsigned channel);
uint8_t motor_reserved_id(unsigned channel,uint32_t id);
const motor_state_t *motor_get(unsigned motor);
void motor_pack_gm(uint8_t id,float current_a,uint8_t data[8]);
void motor_pack_dm(float torque_nm,uint8_t data[8]);
#endif
