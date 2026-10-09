#include "bridge.h"
#include "board.h"
#include "protocol.h"
#include "usbd_cdc_if.h"
#include <string.h>

#define USB_RX_SIZE 2048U
#define QUEUE_SIZE 128U
#define TX_TIMEOUT_MS 100U
#define BUS_OFF_RETRY_MS 1000U
typedef struct { uint16_t length; uint8_t data[PROTO_WIRE_MAX]; } wire_item_t;
typedef struct { wire_item_t items[QUEUE_SIZE]; uint32_t head,tail; } wire_queue_t;
typedef struct { uint16_t sequence; uint8_t active,aborting; uint32_t started,mask; } pending_tx_t;
typedef struct {
    uint32_t nominal,data_rate;
    uint8_t mode,enabled,bus_off;
    uint32_t retry_at,rx,rx_drop,hw_loss,tx_queued,tx_ok,tx_failed,bus_off_events,event_loss;
    pending_tx_t pending[CAN_TX_SLOTS];
    wire_queue_t rx_queue;
} channel_t;

static channel_t channels[CAN_CHANNELS];
static wire_queue_t control_queue;
static proto_parser_t parser;
static uint8_t usb_rx[USB_RX_SIZE],usb_tx[512];
static volatile uint32_t usb_head,usb_tail,session_epoch;
static volatile uint8_t usb_active;
static uint32_t seen_epoch,control_drop,usb_overflow;
static uint16_t usb_tx_length,rx_sequence;
static unsigned next_channel;

void bridge_usb_session(uint8_t active)
{
    if (active==usb_active) return;
    usb_active=active;
    usb_tail=usb_head;
    ++session_epoch;
}
uint8_t bridge_usb_rx_space(void) { return USB_RX_SIZE-(usb_head-usb_tail)>=64U; }
void bridge_usb_rx(const uint8_t *data,uint32_t length)
{
    if (!usb_active) return;
    if (length>USB_RX_SIZE-(usb_head-usb_tail)) { ++usb_overflow; return; }
    for (uint32_t i=0; i<length; ++i) usb_rx[(usb_head+i)%USB_RX_SIZE]=data[i];
    __DMB();
    usb_head+=length;
}

static uint32_t queue_free(const wire_queue_t *q) { return QUEUE_SIZE-(q->head-q->tail); }
static uint8_t queue_push(wire_queue_t *q,const proto_packet_t *packet)
{
    if (!queue_free(q)) return 0;
    wire_item_t *item=&q->items[q->head%QUEUE_SIZE];
    item->length=(uint16_t)proto_encode(packet,item->data);
    ++q->head;
    return 1;
}
static void send_control(const proto_packet_t *packet)
{
    if (usb_active && !queue_push(&control_queue,packet)) ++control_drop;
}
static void tx_result(unsigned ch,pending_tx_t *tx,uint8_t status)
{
    proto_packet_t out={.type=MSG_TX_RESULT,.channel=(uint8_t)(ch+1),.sequence=tx->sequence,.length=5};
    out.payload[0]=status;
    proto_put_u32(out.payload+1,board_micros());
    send_control(&out);
    tx->active=0;
    if (status==STATUS_OK) ++channels[ch].tx_ok;
    else ++channels[ch].tx_failed;
}

static uint8_t restart(unsigned ch,uint8_t report)
{
    channel_t *c=&channels[ch];
    /* Stop 后重新配置会清空硬件 FIFO，避免旧请求在重连或切换速率后继续发送。 */
    uint8_t status=board_can_state(ch,0);
    if (status) return status;
    for (unsigned i=0; i<CAN_TX_SLOTS; ++i) {
        if (c->pending[i].active) {
            if (report) tx_result(ch,&c->pending[i],STATUS_CANCELED);
            else { c->pending[i].active=0; ++c->tx_failed; }
        }
    }
    status=board_can_configure(ch,c->nominal,c->data_rate,c->mode);
    if (!status && c->enabled) status=board_can_state(ch,1);
    if (status) c->enabled=0;
    c->bus_off=0;
    c->rx_drop+=c->rx_queue.head-c->rx_queue.tail;
    c->rx_queue.head=c->rx_queue.tail=0;
    return status;
}

void bridge_init(void)
{
    for (unsigned ch=0; ch<CAN_CHANNELS; ++ch) {
        channel_t *c=&channels[ch];
        c->nominal=1000000;
        c->data_rate=2000000;
        c->enabled=1;
        if (restart(ch,0)!=STATUS_OK) Error_Handler();
    }
}

static uint8_t send_can(const proto_packet_t *p,unsigned ch)
{
    channel_t *c=&channels[ch];
    FDCAN_HandleTypeDef *h=&board_can[ch];
    if (p->length<5 || (p->flags & ~(FLAG_EXT|FLAG_RTR|FLAG_FD|FLAG_BRS))) return STATUS_INVALID;
    uint32_t id=proto_get_u32(p->payload);
    uint8_t length=p->payload[4];
    int dlc=proto_length_to_dlc(length);
    if (dlc<0 || (!(p->flags&FLAG_FD) && length>8) ||
        ((p->flags&FLAG_BRS) && !(p->flags&FLAG_FD)) ||
        ((p->flags&FLAG_FD) && (p->flags&FLAG_RTR)) ||
        id>((p->flags&FLAG_EXT) ? 0x1fffffffU : 0x7ffU) ||
        p->length!=5U+((p->flags&FLAG_RTR) ? 0U : length) ||
        ((p->flags&FLAG_FD) && !(c->mode&1U))) return STATUS_INVALID;
    if (!c->enabled || c->mode==2 || c->mode==3) return STATUS_CLOSED;
    if (c->bus_off || (h->Instance->PSR & FDCAN_PSR_BO)) return STATUS_BUS_OFF;
    unsigned marker;
    for (marker=0; marker<CAN_TX_SLOTS; ++marker) {
        if (c->pending[marker].active && c->pending[marker].sequence==p->sequence) return STATUS_BUSY;
    }
    for (marker=0; marker<CAN_TX_SLOTS; ++marker) if (!c->pending[marker].active) break;
    if (marker==CAN_TX_SLOTS || !HAL_FDCAN_GetTxFifoFreeLevel(h)) return STATUS_BUSY;
    FDCAN_TxHeaderTypeDef header={0};
    header.Identifier=id;
    header.IdType=(p->flags&FLAG_EXT) ? FDCAN_EXTENDED_ID : FDCAN_STANDARD_ID;
    header.TxFrameType=(p->flags&FLAG_RTR) ? FDCAN_REMOTE_FRAME : FDCAN_DATA_FRAME;
    header.DataLength=(uint32_t)dlc;
    header.ErrorStateIndicator=FDCAN_ESI_ACTIVE;
    header.BitRateSwitch=(p->flags&FLAG_BRS) ? FDCAN_BRS_ON : FDCAN_BRS_OFF;
    header.FDFormat=(p->flags&FLAG_FD) ? FDCAN_FD_CAN : FDCAN_CLASSIC_CAN;
    header.TxEventFifoControl=FDCAN_STORE_TX_EVENTS;
    header.MessageMarker=marker;
    uint8_t data[64]={0};
    if (!(p->flags&FLAG_RTR)) memcpy(data,p->payload+5,length);
    if (HAL_FDCAN_AddMessageToTxFifoQ(h,&header,data)!=HAL_OK) return STATUS_HAL_ERROR;
    c->pending[marker]=(pending_tx_t){.active=1,.sequence=p->sequence,.started=HAL_GetTick(),
                                    .mask=HAL_FDCAN_GetLatestTxFifoQRequestBuffer(h)};
    ++c->tx_queued;
    return STATUS_OK;
}

static void dispatch(const proto_packet_t *p)
{
    proto_packet_t out={.type=MSG_ACK,.channel=p->channel,.sequence=p->sequence,.length=2};
    out.payload[0]=p->type;
    uint8_t status=STATUS_INVALID;
    if (p->type==CMD_INFO && p->channel==0 && !p->flags && !p->length) {
        static const uint8_t info[]={1,0,0,3,64,16,0x0f,0,'D','M','-','H','7','2','3'};
        memcpy(out.payload+2,info,sizeof(info));
        out.length+=sizeof(info);
        status=STATUS_OK;
    } else if (p->type==CMD_PING && p->channel==0 && !p->flags && p->length<=78) {
        memcpy(out.payload+2,p->payload,p->length);
        out.length+=p->length;
        status=STATUS_OK;
    } else if (p->channel>=1 && p->channel<=CAN_CHANNELS) {
        unsigned ch=p->channel-1U;
        channel_t *c=&channels[ch];
        if (p->type==CMD_TX) status=send_can(p,ch);
        else if (!p->flags && p->type==CMD_CONFIG && p->length==9) {
            uint32_t nominal=proto_get_u32(p->payload),data=proto_get_u32(p->payload+4);
            uint8_t mode=p->payload[8];
            /* 配置只允许在关闭通道后修改，避免悄悄丢弃在途帧。 */
            if (c->enabled) status=STATUS_BUSY;
            else {
                status=board_can_configure(ch,nominal,data,mode);
                if (!status) { c->nominal=nominal; c->data_rate=data; c->mode=mode; c->bus_off=0; }
            }
        } else if (!p->flags && p->type==CMD_STATE && p->length==1 && p->payload[0]<=1) {
            if (c->enabled && p->payload[0] && !c->bus_off) status=STATUS_OK;
            else {
                c->enabled=p->payload[0];
                status=restart(ch,1);
            }
        } else if (!p->flags && p->type==CMD_STATS && !p->length) {
            FDCAN_ProtocolStatusTypeDef ps={0};
            FDCAN_ErrorCountersTypeDef ec={0};
            HAL_FDCAN_GetProtocolStatus(&board_can[ch],&ps);
            HAL_FDCAN_GetErrorCounters(&board_can[ch],&ec);
            uint32_t values[]={c->nominal,c->data_rate,c->mode,c->enabled,ps.BusOff,
                ec.TxErrorCnt,ec.RxErrorCnt,c->rx,c->rx_drop,c->hw_loss,
                c->tx_queued,c->tx_ok,c->tx_failed,c->bus_off_events,c->event_loss,
                parser.errors,control_drop,usb_overflow};
            for (unsigned i=0; i<sizeof(values)/sizeof(values[0]); ++i) proto_put_u32(out.payload+2+i*4,values[i]);
            out.length=2+sizeof(values);
            status=STATUS_OK;
        }
    }
    out.payload[1]=status;
    send_control(&out);
}

static void poll_can(unsigned ch)
{
    channel_t *c=&channels[ch];
    FDCAN_HandleTypeDef *h=&board_can[ch];
    if (!c->enabled) return;
    uint32_t now=HAL_GetTick();
    if (__HAL_FDCAN_GET_FLAG(h,FDCAN_FLAG_RX_FIFO0_MESSAGE_LOST)) {
        ++c->hw_loss;
        __HAL_FDCAN_CLEAR_FLAG(h,FDCAN_FLAG_RX_FIFO0_MESSAGE_LOST);
    }
    if (h->Instance->IR & FDCAN_IR_TEFL) {
        ++c->event_loss;
        h->Instance->IR=FDCAN_IR_TEFL;
    }
    /* 收帧、完成事件均限制每轮工作量，三路满载时仍能公平服务 USB 和其他通道。 */
    for (unsigned i=0; i<16 && HAL_FDCAN_GetRxFifoFillLevel(h,FDCAN_RX_FIFO0); ++i) {
        FDCAN_RxHeaderTypeDef rx;
        uint8_t data[64];
        if (HAL_FDCAN_GetRxMessage(h,FDCAN_RX_FIFO0,&rx,data)!=HAL_OK) break;
        ++c->rx;
        if (!usb_active) { ++c->rx_drop; continue; }
        proto_packet_t out={.type=MSG_RX,.channel=(uint8_t)(ch+1),.sequence=rx_sequence++};
        if (rx.IdType==FDCAN_EXTENDED_ID) out.flags|=FLAG_EXT;
        if (rx.RxFrameType==FDCAN_REMOTE_FRAME) out.flags|=FLAG_RTR;
        if (rx.FDFormat==FDCAN_FD_CAN) out.flags|=FLAG_FD;
        if (rx.BitRateSwitch==FDCAN_BRS_ON) out.flags|=FLAG_BRS;
        if (rx.ErrorStateIndicator==FDCAN_ESI_PASSIVE) out.flags|=FLAG_ESI;
        uint8_t length=proto_dlc_to_length((uint8_t)rx.DataLength);
        if (!(out.flags&FLAG_FD) && length>8) length=8;
        proto_put_u32(out.payload,rx.Identifier);
        proto_put_u32(out.payload+4,board_micros());
        out.payload[8]=length;
        out.length=9+((out.flags&FLAG_RTR) ? 0U : length);
        if (!(out.flags&FLAG_RTR)) memcpy(out.payload+9,data,length);
        if (!queue_push(&c->rx_queue,&out)) ++c->rx_drop;
    }
    for (unsigned i=0; i<16 && (h->Instance->TXEFS & FDCAN_TXEFS_EFFL); ++i) {
        FDCAN_TxEventFifoTypeDef event;
        if (HAL_FDCAN_GetTxEvent(h,&event)!=HAL_OK) break;
        unsigned marker=event.MessageMarker;
        if (marker<CAN_TX_SLOTS && c->pending[marker].active) tx_result(ch,&c->pending[marker],STATUS_OK);
    }
    for (unsigned i=0; i<CAN_TX_SLOTS; ++i) {
        pending_tx_t *tx=&c->pending[i];
        if (!tx->active) continue;
        if (!HAL_FDCAN_IsTxBufferMessagePending(h,tx->mask)) {
            /* TXBTO 在事件丢失或取消竞态下仍能确认是否真实完成过发送。 */
            tx_result(ch,tx,(h->Instance->TXBTO & tx->mask) ? STATUS_OK : STATUS_TIMEOUT);
        } else if (!tx->aborting && now-tx->started>=TX_TIMEOUT_MS) {
            if (HAL_FDCAN_AbortTxRequest(h,tx->mask)==HAL_OK) tx->aborting=1;
        }
    }
    if (h->Instance->PSR & FDCAN_PSR_BO) {
        if (!c->bus_off) {
            c->bus_off=1;
            c->retry_at=now+BUS_OFF_RETRY_MS;
            ++c->bus_off_events;
        }
        if ((int32_t)(now-c->retry_at)>=0) restart(ch,1);
    } else c->bus_off=0;
}

static uint8_t append_queue(wire_queue_t *q)
{
    if (q->head==q->tail) return 0;
    wire_item_t *item=&q->items[q->tail%QUEUE_SIZE];
    if (usb_tx_length+item->length>sizeof(usb_tx)) return 0;
    memcpy(usb_tx+usb_tx_length,item->data,item->length);
    usb_tx_length+=item->length;
    ++q->tail;
    return 1;
}

static void poll_usb_tx(void)
{
    if (!usb_active || !cdc_tx_ready()) return;
    if (!usb_tx_length) {
        unsigned control_budget=4;
        while (control_budget-- && append_queue(&control_queue)) {}
        for (unsigned round=0; round<6; ++round) {
            unsigned ch=next_channel++%CAN_CHANNELS;
            append_queue(&channels[ch].rx_queue);
        }
        while (append_queue(&control_queue)) {}
    }
    /* usb_tx 直到 TxState 清零才再次写入，USB_BUSY 时保留整批数据重试。 */
    if (usb_tx_length && CDC_Transmit_HS(usb_tx,usb_tx_length)==USBD_OK) usb_tx_length=0;
}

void bridge_poll(void)
{
    if (seen_epoch!=session_epoch) {
        seen_epoch=session_epoch;
        parser.used=0;
        parser.discarding=false;
        control_queue.head=control_queue.tail=0;
        usb_tx_length=0;
        for (unsigned ch=0; ch<CAN_CHANNELS; ++ch) restart(ch,0);
    }
    for (unsigned ch=0; ch<CAN_CHANNELS; ++ch) poll_can(ch);
    unsigned budget=256;
    while (budget-- && usb_active && queue_free(&control_queue)>=CAN_CHANNELS*CAN_TX_SLOTS+2) {
        uint32_t irq=__get_PRIMASK();
        __disable_irq();
        if (seen_epoch!=session_epoch || usb_tail==usb_head) { __set_PRIMASK(irq); break; }
        uint8_t byte=usb_rx[usb_tail++%USB_RX_SIZE];
        __set_PRIMASK(irq);
        proto_packet_t p;
        if (proto_feed(&parser,byte,&p)) dispatch(&p);
    }
    cdc_poll();
    poll_usb_tx();
}
