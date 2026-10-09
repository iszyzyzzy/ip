#include "motor.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static uint8_t frames[2][8],urgent[2],ready=1,send_result=MOTOR_SEND_OK;
static uint32_t identifiers[2];
static unsigned cancelled[2];
static uint8_t send_stub(unsigned motor,uint16_t id,const uint8_t data[8],uint8_t immediate)
{
    assert(motor<2);
    memcpy(frames[motor],data,8);
    identifiers[motor]=id; urgent[motor]=immediate;
    return send_result;
}
static uint8_t ready_stub(void) { return ready; }
static void cancel_stub(unsigned motor) { ++cancelled[motor]; }
static void reset(void)
{
    ready=1; send_result=MOTOR_SEND_OK;
    memset(frames,0,sizeof(frames)); memset(cancelled,0,sizeof(cancelled));
    motor_init(send_stub,ready_stub,cancel_stub);
}
static void put_float(uint8_t *out,float value) { uint32_t bits; memcpy(&bits,&value,4); proto_put_u32(out,bits); }
static uint8_t command(uint8_t type,const uint8_t *data,unsigned length,uint32_t now)
{
    proto_packet_t p={0},response={0};
    p.type=type; p.channel=MOTOR_CAN_CHANNEL; p.length=(uint16_t)length;
    if (length) memcpy(p.payload,data,length);
    return motor_command(&p,&response,now);
}
static uint8_t position(uint8_t mask,float gm,float dm,uint32_t now)
{
    uint8_t data[9]={mask}; put_float(data+1,gm); put_float(data+5,dm);
    return command(CMD_MOTOR_POSITION,data,9,now);
}
static void gm_feedback(uint16_t angle,int16_t rpm,uint32_t now)
{
    uint8_t data[8]={(uint8_t)(angle>>8),(uint8_t)angle,(uint8_t)((uint16_t)rpm>>8),(uint8_t)rpm,0,0,30,0};
    motor_receive(MOTOR_CAN_CHANNEL,0x204+MOTOR_GM_ID,0,data,8,now);
}
static void dm_feedback(unsigned state,uint32_t now)
{
    uint8_t data[8]={(uint8_t)((state<<4)|MOTOR_DM_ID),0x7f,0xff,0x7f,0xf7,0xff,31,32};
    motor_receive(MOTOR_CAN_CHANNEL,MOTOR_DM_FEEDBACK_ID,0,data,8,now);
}
static void both_feedback(uint32_t now) { gm_feedback(100,0,now); dm_feedback(1,now); }

static void check_wire_format(void)
{
    uint8_t data[8];
    motor_pack_gm(1,3,data); assert(data[0]==0x40 && data[1]==0);
    for (unsigned i=2;i<8;++i) assert(data[i]==0);
    motor_pack_gm(7,-5,data); assert(data[4]==0xc0 && data[5]==0 && data[0]==0);
    motor_pack_dm(-10,data);
    assert(data[0]==0x7f && data[1]==0xff && data[2]==0x7f && data[3]==0xf0);
    assert(data[4]==0 && data[5]==0 && data[6]==0 && data[7]==0);
    motor_pack_dm(10,data); assert(data[6]==0x0f && data[7]==0xff);
    assert(motor_reserved_id(MOTOR_CAN_CHANNEL,0x1fe));
    assert(motor_reserved_id(MOTOR_CAN_CHANNEL,MOTOR_DM_ID));
    assert(!motor_reserved_id(MOTOR_CAN_CHANNEL%3+1,0x1fe));
    assert(!motor_reserved_id(MOTOR_CAN_CHANNEL,0x123));
}
static void check_feedback_reference(void)
{
    reset(); gm_feedback(8190,-60,1); gm_feedback(2,-60,2);
    assert(fabsf(motor_get(0)->position-4*6.2831853f/8192)<0.00001f);
    assert(fabsf(motor_get(0)->velocity+6.2831853f)<0.00001f);
    assert(motor_get(0)->reference_valid);
    gm_feedback(5,0,103);
    assert(!motor_get(0)->reference_valid);
    assert(position(1,1,0,103)==STATUS_MOTOR_FAULT);
    uint8_t mask=1;
    assert(command(CMD_MOTOR_ZERO,&mask,1,103)==STATUS_OK);
    assert(motor_get(0)->reference_valid && motor_get(0)->position==0);
    dm_feedback(1,103);
    assert(motor_get(1)->driver_state==1 && motor_get(1)->temperature==32);
    assert(fabsf(motor_get(1)->velocity)<0.008 && fabsf(motor_get(1)->effort)<0.003);
    uint8_t bad[8]={0};
    motor_receive(MOTOR_CAN_CHANNEL%3+1,0x204+MOTOR_GM_ID,0,bad,8,104);
    motor_receive(MOTOR_CAN_CHANNEL,0x204+MOTOR_GM_ID,FLAG_FD,bad,8,104);
    assert(motor_get(0)->feedback_ms==103);
}
static void check_independent_control(void)
{
    reset(); both_feedback(0);
    assert(position(3,1,-1,0)==STATUS_OK);
    motor_poll(1,1);
    assert(motor_get(0)->state==MOTOR_RUNNING && motor_get(1)->state==MOTOR_RUNNING);
    assert(motor_get(0)->output>0 && motor_get(0)->output<=0.5f);
    assert(motor_get(1)->output<0 && motor_get(1)->output>=-0.5f);
    assert(motor_get(1)->integral==0);
    assert(identifiers[0]==(MOTOR_GM_ID<=4 ? 0x1feU:0x2feU) && identifiers[1]==MOTOR_DM_ID);
    assert(!urgent[0] && !urgent[1]);
    assert(position(3,2,NAN,1)==STATUS_INVALID);
    assert(motor_get(0)->target==1 && motor_get(1)->target==-1);
    assert(position(1,2,NAN,1)==STATUS_OK);
    assert(motor_get(1)->target==-1);
    uint8_t mask=1;
    assert(command(CMD_MOTOR_STOP,&mask,1,1)==STATUS_OK);
    motor_poll(2,1);
    assert(motor_get(0)->state==MOTOR_STOPPED && motor_get(1)->state==MOTOR_RUNNING);
    assert(urgent[0] && !urgent[1]);
    for (unsigned i=0;i<8;++i) assert(frames[0][i]==0);
    assert(cancelled[0]>0);
    motor_poll(3,0);
    assert(motor_get(1)->fault==MOTOR_FAULT_HOST && frames[1][7]==0xfd);
    motor_bus_reset(MOTOR_CAN_CHANNEL); assert(motor_get(1)->fault==MOTOR_FAULT_HOST);
    both_feedback(4); motor_poll(4,1);
    assert(motor_get(1)->state==MOTOR_FAULT);
}
static void check_faults_and_arming(void)
{
    reset(); assert(position(1,1,0,0)==STATUS_CLOSED);
    assert(position(2,0,1,0)==STATUS_OK); motor_poll(1,1);
    assert(motor_get(1)->state==MOTOR_STARTING && frames[1][7]==0xfc);
    dm_feedback(1,2); motor_poll(2,1); assert(motor_get(1)->state==MOTOR_RUNNING);
    for (unsigned t=3;t<=103;++t) motor_poll(t,1);
    assert(motor_get(1)->fault==MOTOR_FAULT_FEEDBACK && frames[1][7]==0xfd);
    reset(); both_feedback(0); assert(position(3,1,1,0)==STATUS_OK);
    motor_poll(20,1);
    assert(motor_get(0)->fault==MOTOR_FAULT_TIMING && motor_get(1)->fault==MOTOR_FAULT_TIMING);
    reset(); both_feedback(0); assert(position(3,1,1,0)==STATUS_OK);
    motor_bus_reset(MOTOR_CAN_CHANNEL%3+1); assert(motor_get(0)->state==MOTOR_RUNNING);
    motor_bus_reset(MOTOR_CAN_CHANNEL); assert(motor_get(0)->fault==MOTOR_FAULT_BUS);
    reset(); both_feedback(0); assert(position(2,0,1,0)==STATUS_OK);
    dm_feedback(8,1); motor_poll(1,1); assert(motor_get(1)->fault==MOTOR_FAULT_DRIVER);
    reset(); both_feedback(0); assert(position(1,1,0,0)==STATUS_OK);
    uint8_t hot[8]={0,100,0,0,0,0,80,0};
    motor_receive(MOTOR_CAN_CHANNEL,0x204+MOTOR_GM_ID,0,hot,8,1);
    motor_poll(1,1); assert(motor_get(0)->fault==MOTOR_FAULT_TEMPERATURE);
    reset(); both_feedback(0); assert(position(1,1,0,0)==STATUS_OK);
    send_result=MOTOR_SEND_BUSY;
    for (unsigned t=1;t<=21;++t) motor_poll(t,1);
    assert(motor_get(0)->fault==MOTOR_FAULT_TX);
}
static void check_tuning_and_status(void)
{
    reset(); uint8_t data[25]={1};
    const float tuning[6]={8,0.2f,0.3f,3,0.4f,10};
    for (unsigned i=0;i<6;++i) put_float(data+1+i*4,tuning[i]);
    assert(command(CMD_MOTOR_TUNE,data,25,0)==STATUS_OK);
    assert(motor_get(0)->tuning.max_output==0.4f);
    both_feedback(0); assert(position(1,1,0,0)==STATUS_OK);
    assert(command(CMD_MOTOR_TUNE,data,25,0)==STATUS_BUSY);
    motor_stop(1,0); put_float(data+17,4);
    assert(command(CMD_MOTOR_TUNE,data,25,0)==STATUS_INVALID);
    proto_packet_t p={0},out={0}; p.type=CMD_MOTOR_STATUS; p.channel=MOTOR_CAN_CHANNEL;
    assert(motor_command(&p,&out,5)==STATUS_OK && out.length==70);
    assert(out.payload[2]==1 && out.payload[34]==2 && proto_get_u32(out.payload+26)==5);
    p.channel=MOTOR_CAN_CHANNEL%3+1; assert(motor_command(&p,&out,5)==STATUS_INVALID);
    /* STOP 在第一次位置指令之前也要发送零电流和达妙失能指令。 */
    reset(); motor_stop(3,0); motor_poll(1,1); assert(frames[1][7]==0xfd);
}
int main(void)
{
    check_wire_format(); check_feedback_reference(); check_independent_control();
    check_faults_and_arming(); check_tuning_and_status();
    puts("电机控制检查通过：封包、独立闭环、零点恢复及故障停止");
    return 0;
}
