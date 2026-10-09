#include "motor.h"
#include <math.h>
#include <string.h>

_Static_assert(MOTOR_CAN_CHANNEL>=1 && MOTOR_CAN_CHANNEL<=3,"电机 CAN 通道必须为 1～3");
_Static_assert(MOTOR_GM_ID>=1 && MOTOR_GM_ID<=7,"GM6020 ID 必须为 1～7");
_Static_assert(MOTOR_DM_ID>=1 && MOTOR_DM_ID<=15,"DM4310 ID 必须为 1～15，以便验证反馈低四位 ID");
_Static_assert(MOTOR_DM_FEEDBACK_ID<=0x7ff && MOTOR_DM_FEEDBACK_ID!=MOTOR_DM_ID &&
               MOTOR_DM_FEEDBACK_ID!=0x204+MOTOR_GM_ID,"电机反馈 ID 不能冲突");
#define PI_F 3.14159265358979323846f
#define GM_COMMAND_ID (MOTOR_GM_ID<=4 ? 0x1feU : 0x2feU)
static motor_state_t motors[MOTOR_COUNT];
static motor_send_fn send_frame;
static motor_bus_ready_fn bus_ready;
static motor_cancel_fn cancel_frame;
static uint32_t last_poll_ms,missed_ticks;
static uint32_t tx_busy_since[MOTOR_COUNT];
static uint8_t tx_busy[MOTOR_COUNT];

static float clamp(float value,float limit)
{
    if (value>limit) return limit;
    if (value<-limit) return -limit;
    return value;
}
static float get_float(const uint8_t *data)
{
    uint32_t bits=proto_get_u32(data);
    float value;
    memcpy(&value,&bits,sizeof(value));
    return value;
}
static void put_float(uint8_t *data,float value)
{
    uint32_t bits;
    memcpy(&bits,&value,sizeof(bits));
    proto_put_u32(data,bits);
}

void motor_pack_gm(uint8_t id,float current,uint8_t data[8])
{
    memset(data,0,8);
    int32_t command=(int32_t)(clamp(current,3.0f)*(16384.0f/3.0f));
    uint16_t raw=(uint16_t)(int16_t)command;
    unsigned slot=(id-1U)%4U;
    data[slot*2]=(uint8_t)(raw>>8);
    data[slot*2+1]=(uint8_t)raw;
}

void motor_pack_dm(float torque,uint8_t data[8])
{
    /* 将电机内置 Kp/Kd 置零，位置和速度环只在 H723 上运行，避免重复闭环。 */
    uint16_t p=32767U,v=2047U;
    uint16_t t=(uint16_t)((clamp(torque,MOTOR_DM_T_MAX)+MOTOR_DM_T_MAX)*4095.0f/(2.0f*MOTOR_DM_T_MAX));
    data[0]=(uint8_t)(p>>8); data[1]=(uint8_t)p;
    data[2]=(uint8_t)(v>>4); data[3]=(uint8_t)((v&15U)<<4);
    data[4]=0; data[5]=0; data[6]=(uint8_t)(t>>8); data[7]=(uint8_t)t;
}

static void special_dm(uint8_t command,uint8_t data[8])
{
    memset(data,0xff,8);
    data[7]=command;
}

void motor_init(motor_send_fn send,motor_bus_ready_fn ready,motor_cancel_fn cancel)
{
    memset(motors,0,sizeof(motors));
    memset(tx_busy,0,sizeof(tx_busy));
    last_poll_ms=missed_ticks=0;
    send_frame=send; bus_ready=ready; cancel_frame=cancel;
    motors[MOTOR_GM].tuning=(motor_tuning_t){8.0f,0.12f,0.30f,3.0f,0.5f,12.5f};
    motors[MOTOR_DM].tuning=(motor_tuning_t){6.0f,0.25f,0.50f,3.0f,0.5f,12.5f};
}
const motor_state_t *motor_get(unsigned motor) { return motor<MOTOR_COUNT ? &motors[motor] : NULL; }

void motor_stop(uint8_t mask,uint8_t fault)
{
    for (unsigned i=0; i<MOTOR_COUNT; ++i) {
        if (!(mask & (1U<<i))) continue;
        motor_state_t *m=&motors[i];
        if (fault && !m->claimed) continue;
        if (!fault) m->claimed=1;
        m->state=fault ? MOTOR_FAULT : MOTOR_STOPPED;
        m->fault=fault;
        m->output=m->integral=0;
        m->target=m->position;
        tx_busy[i]=0;
        if (cancel_frame) cancel_frame(i);
    }
}
void motor_bus_reset(unsigned channel)
{
    if (channel!=MOTOR_CAN_CHANNEL) return;
    for (unsigned i=0;i<MOTOR_COUNT;++i) {
        if (motors[i].state==MOTOR_STARTING || motors[i].state==MOTOR_RUNNING)
            motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_BUS);
    }
}

uint8_t motor_reserved_id(unsigned channel,uint32_t id)
{
    if (channel!=MOTOR_CAN_CHANNEL) return 0;
    /* 同一条总线不能同时接受上位机原始指令和本地闭环对同一执行器的控制。 */
    return id==0x1feU || id==0x2feU || id==0x1ffU || id==0x2ffU ||
           id==MOTOR_DM_ID || id==MOTOR_DM_ID+0x100U ||
           id==MOTOR_DM_ID+0x200U || id==MOTOR_DM_ID+0x300U;
}

static void update_position(motor_state_t *m,float raw,float period,uint32_t now)
{
    if (!m->have_feedback) {
        m->absolute=raw;
        m->origin=raw;
        m->reference_valid=1;
    } else {
        if (now-m->feedback_ms>MOTOR_FEEDBACK_TIMEOUT_MS) {
            /* 单圈反馈在失联期间可能跨过多圈，不能猜测圈数后继续追旧目标。 */
            m->reference_valid=0;
        }
        float delta=raw-m->previous_raw;
        if (delta>period*0.5f) delta-=period;
        if (delta<-period*0.5f) delta+=period;
        m->absolute+=delta;
    }
    m->previous_raw=raw;
    m->position=m->absolute-m->origin;
    m->have_feedback=1;
    m->feedback_ms=now;
}

void motor_receive(unsigned channel,uint32_t id,uint8_t flags,const uint8_t *data,uint8_t length,uint32_t now)
{
    if (channel!=MOTOR_CAN_CHANNEL || flags || length!=8) return;
    if (id==0x204U+MOTOR_GM_ID) {
        uint16_t angle=(uint16_t)((data[0]<<8)|data[1]);
        if (angle>=8192) return;
        motor_state_t *m=&motors[MOTOR_GM];
        update_position(m,angle*(2.0f*PI_F/8192.0f),2.0f*PI_F,now);
        m->velocity=(int16_t)((data[2]<<8)|data[3])*(2.0f*PI_F/60.0f);
        m->effort=(int16_t)((data[4]<<8)|data[5])*(3.0f/16384.0f);
        m->temperature=data[6];
    } else if (id==MOTOR_DM_FEEDBACK_ID && (data[0]&15U)==MOTOR_DM_ID) {
        motor_state_t *m=&motors[MOTOR_DM];
        uint16_t p=(uint16_t)((data[1]<<8)|data[2]);
        uint16_t v=(uint16_t)((data[3]<<4)|(data[4]>>4));
        uint16_t t=(uint16_t)(((data[4]&15U)<<8)|data[5]);
        update_position(m,p*(2.0f*MOTOR_DM_P_MAX/65535.0f)-MOTOR_DM_P_MAX,2.0f*MOTOR_DM_P_MAX,now);
        m->velocity=v*(2.0f*MOTOR_DM_V_MAX/4095.0f)-MOTOR_DM_V_MAX;
        m->effort=t*(2.0f*MOTOR_DM_T_MAX/4095.0f)-MOTOR_DM_T_MAX;
        m->driver_state=data[0]>>4;
        m->temperature=data[6]>data[7] ? data[6] : data[7];
    }
}

static uint8_t online(const motor_state_t *m,uint32_t now)
{
    return m->have_feedback && now-m->feedback_ms<=MOTOR_FEEDBACK_TIMEOUT_MS;
}

static uint8_t validate_position(unsigned i,float position,uint32_t now)
{
    const motor_state_t *m=&motors[i];
    if (!isfinite(position) || fabsf(position)>m->tuning.max_position) return STATUS_INVALID;
    if (online(m,now) && !m->reference_valid) return STATUS_MOTOR_FAULT;
    if (online(m,now) && m->temperature>=MOTOR_TEMPERATURE_LIMIT) return STATUS_MOTOR_FAULT;
    if (i==MOTOR_DM && online(m,now) && m->driver_state>1) return STATUS_MOTOR_FAULT;
    if (i==MOTOR_GM && !online(m,now)) return STATUS_CLOSED;
    return STATUS_OK;
}

static void status_payload(proto_packet_t *response,uint32_t now)
{
    response->length=70;
    for (unsigned i=0; i<MOTOR_COUNT; ++i) {
        const motor_state_t *m=&motors[i];
        uint8_t *out=response->payload+2+i*32;
        out[0]=(uint8_t)(i+1); out[1]=m->state; out[2]=m->fault; out[3]=m->temperature;
        put_float(out+4,m->position); put_float(out+8,m->velocity); put_float(out+12,m->effort);
        put_float(out+16,m->target); put_float(out+20,m->output);
        proto_put_u32(out+24,m->have_feedback ? now-m->feedback_ms : 0xffffffffU);
        proto_put_u32(out+28,m->tx_failures);
    }
    proto_put_u32(response->payload+66,missed_ticks);
}

uint8_t motor_command(const proto_packet_t *p,proto_packet_t *out,uint32_t now)
{
    if (p->channel!=MOTOR_CAN_CHANNEL || p->flags) return STATUS_INVALID;
    if (p->type==CMD_MOTOR_STATUS && !p->length) {
        status_payload(out,now);
        return STATUS_OK;
    }
    if (p->type==CMD_MOTOR_POSITION && p->length==9) {
        uint8_t mask=p->payload[0];
        if (!mask || mask>3 || !bus_ready()) return mask && mask<=3 ? STATUS_CLOSED : STATUS_INVALID;
        float targets[2]={get_float(p->payload+1),get_float(p->payload+5)};
        for (unsigned i=0; i<MOTOR_COUNT; ++i) {
            if (mask&(1U<<i)) {
                uint8_t status=validate_position(i,targets[i],now);
                if (status) return status;
            }
        }
        for (unsigned i=0; i<MOTOR_COUNT; ++i) {
            if (!(mask&(1U<<i))) continue;
            motor_state_t *m=&motors[i];
            m->target=targets[i];
            if (m->state!=MOTOR_RUNNING && m->state!=MOTOR_STARTING) {
                m->state=i==MOTOR_GM ? MOTOR_RUNNING : MOTOR_STARTING;
                m->started_ms=now;
                m->claimed=1;
                m->integral=m->output=0;
                m->fault=MOTOR_FAULT_NONE;
                tx_busy[i]=0;
                if (cancel_frame) cancel_frame(i);
            }
        }
        return STATUS_OK;
    }
    if (p->type==CMD_MOTOR_STOP && p->length==1 && p->payload[0]>=1 && p->payload[0]<=3) {
        motor_stop(p->payload[0],MOTOR_FAULT_NONE);
        return STATUS_OK;
    }
    if (p->type==CMD_MOTOR_ZERO && p->length==1 && p->payload[0]>=1 && p->payload[0]<=3) {
        uint8_t mask=p->payload[0];
        for (unsigned i=0; i<MOTOR_COUNT; ++i) {
            if (!(mask&(1U<<i))) continue;
            if (motors[i].state==MOTOR_RUNNING || motors[i].state==MOTOR_STARTING) return STATUS_BUSY;
            if (!online(&motors[i],now)) return STATUS_CLOSED;
        }
        for (unsigned i=0; i<MOTOR_COUNT; ++i) {
            if (!(mask&(1U<<i))) continue;
            motors[i].origin=motors[i].absolute;
            motors[i].position=motors[i].target=0;
            motors[i].reference_valid=1;
        }
        return STATUS_OK;
    }
    if (p->type==CMD_MOTOR_TUNE && p->length==25 && p->payload[0]>=1 && p->payload[0]<=2) {
        unsigned i=p->payload[0]-1U;
        motor_state_t *m=&motors[i];
        if (m->state==MOTOR_RUNNING || m->state==MOTOR_STARTING) return STATUS_BUSY;
        float values[6];
        for (unsigned k=0; k<6; ++k) {
            values[k]=get_float(p->payload+1+k*4);
            if (!isfinite(values[k]) || values[k]<0) return STATUS_INVALID;
        }
        float output_limit=i==MOTOR_GM ? 3.0f : MOTOR_DM_T_MAX;
        if (values[0]>100 || values[1]>100 || values[2]>100 || values[3]<=0 || values[3]>30 ||
            values[4]<=0 || values[4]>output_limit || values[5]<=0 || values[5]>1000) return STATUS_INVALID;
        m->tuning=(motor_tuning_t){values[0],values[1],values[2],values[3],values[4],values[5]};
        m->integral=0;
        return STATUS_OK;
    }
    return STATUS_INVALID;
}

static float controller(motor_state_t *m,float dt)
{
    float velocity=clamp(m->tuning.position_kp*(m->target-m->position),m->tuning.max_velocity);
    float error=velocity-m->velocity;
    float integral=clamp(m->integral+m->tuning.velocity_ki*error*dt,m->tuning.max_output);
    float value=m->tuning.velocity_kp*error+integral;
    /* 饱和时只允许积分朝解除饱和的方向变化，避免大角度指令积累过多电流。 */
    if (fabsf(value)<=m->tuning.max_output || value*error<0) m->integral=integral;
    return clamp(m->tuning.velocity_kp*error+m->integral,m->tuning.max_output);
}

void motor_poll(uint32_t now,uint8_t session_active)
{
    uint32_t elapsed=now-last_poll_ms;
    if (elapsed<MOTOR_CONTROL_PERIOD_MS) return;
    last_poll_ms=now;
    if (elapsed>1) missed_ticks+=elapsed-1U;
    for (unsigned i=0; i<MOTOR_COUNT; ++i) {
        motor_state_t *m=&motors[i];
        if (!m->claimed) continue;
        if (m->state==MOTOR_STARTING || m->state==MOTOR_RUNNING) {
            if (!session_active) motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_HOST);
            else if (!bus_ready()) motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_BUS);
            else if (elapsed>10) motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_TIMING);
            else if (online(m,now) && !m->reference_valid) motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_REFERENCE);
            else if (online(m,now) && m->temperature>=MOTOR_TEMPERATURE_LIMIT) motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_TEMPERATURE);
            else if (i==MOTOR_DM && online(m,now) && m->driver_state>1) motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_DRIVER);
            else if (m->state==MOTOR_RUNNING && (!online(m,now) || (i==MOTOR_DM && m->driver_state!=1)))
                motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_FEEDBACK);
            else if (m->state==MOTOR_STARTING && now-m->started_ms>MOTOR_ARM_TIMEOUT_MS)
                motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_FEEDBACK);
            else if (m->state==MOTOR_STARTING && online(m,now) && m->driver_state==1)
                m->state=MOTOR_RUNNING;
        }
        uint8_t data[8],urgent=0;
        if (m->state==MOTOR_RUNNING) {
            m->output=controller(m,elapsed*0.001f);
            if (i==MOTOR_GM) motor_pack_gm(MOTOR_GM_ID,m->output,data);
            else motor_pack_dm(m->output,data);
        } else {
            m->output=m->integral=0;
            if (i==MOTOR_GM) motor_pack_gm(MOTOR_GM_ID,0,data);
            else special_dm(m->state==MOTOR_STARTING ? 0xfc : 0xfd,data);
            urgent=1;
        }
        uint8_t result=send_frame(i,i==MOTOR_GM ? GM_COMMAND_ID : MOTOR_DM_ID,data,urgent);
        if (result==MOTOR_SEND_OK) tx_busy[i]=0;
        else {
            ++m->tx_failures;
            if (!tx_busy[i]) { tx_busy[i]=1; tx_busy_since[i]=now; }
            if (now-tx_busy_since[i]>=20 && (m->state==MOTOR_RUNNING || m->state==MOTOR_STARTING))
                motor_stop((uint8_t)(1U<<i),MOTOR_FAULT_TX);
        }
    }
}
