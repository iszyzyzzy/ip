#ifndef PROTOCOL_H
#define PROTOCOL_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#define PROTO_VERSION 1U
#define PROTO_PAYLOAD_MAX 80U
#define PROTO_RAW_MAX (PROTO_PAYLOAD_MAX + 10U)
#define PROTO_WIRE_MAX (PROTO_RAW_MAX + 2U)
enum { CMD_INFO=1, CMD_CONFIG=2, CMD_STATE=3, CMD_TX=4, CMD_STATS=5, CMD_PING=6,
       CMD_MOTOR_POSITION=0x10, CMD_MOTOR_STATUS=0x11, CMD_MOTOR_STOP=0x12,
       CMD_MOTOR_TUNE=0x13, CMD_MOTOR_ZERO=0x14,
       MSG_ACK=0x80, MSG_RX=0x81, MSG_TX_RESULT=0x82 };
enum { STATUS_OK=0, STATUS_INVALID=1, STATUS_BUSY=2, STATUS_CLOSED=3,
       STATUS_HAL_ERROR=4, STATUS_TIMEOUT=5, STATUS_CANCELED=6, STATUS_BUS_OFF=7,
       STATUS_MOTOR_FAULT=8 };
enum { FLAG_EXT=1, FLAG_RTR=2, FLAG_FD=4, FLAG_BRS=8, FLAG_ESI=16 };
typedef struct {
    uint8_t type, channel, flags;
    uint16_t sequence, length;
    uint8_t payload[PROTO_PAYLOAD_MAX];
} proto_packet_t;
typedef struct {
    uint8_t encoded[PROTO_WIRE_MAX];
    uint16_t used;
    bool discarding;
    uint32_t errors;
} proto_parser_t;
uint16_t proto_crc16(const uint8_t *data, size_t length);
size_t proto_encode(const proto_packet_t *packet, uint8_t *wire);
bool proto_feed(proto_parser_t *parser, uint8_t byte, proto_packet_t *packet);
uint16_t proto_get_u16(const uint8_t *p);
uint32_t proto_get_u32(const uint8_t *p);
void proto_put_u16(uint8_t *p, uint16_t value);
void proto_put_u32(uint8_t *p, uint32_t value);
int proto_length_to_dlc(uint8_t length);
uint8_t proto_dlc_to_length(uint8_t dlc);
#endif
