#include "protocol.h"
#include <string.h>

uint16_t proto_get_u16(const uint8_t *p) { return (uint16_t)(p[0] | ((uint16_t)p[1] << 8)); }
uint32_t proto_get_u32(const uint8_t *p) { return proto_get_u16(p) | ((uint32_t)proto_get_u16(p+2) << 16); }
void proto_put_u16(uint8_t *p, uint16_t v) { p[0]=(uint8_t)v; p[1]=(uint8_t)(v>>8); }
void proto_put_u32(uint8_t *p, uint32_t v) { proto_put_u16(p,(uint16_t)v); proto_put_u16(p+2,(uint16_t)(v>>16)); }

uint16_t proto_crc16(const uint8_t *data, size_t length)
{
    uint16_t crc=0xffff;
    for (size_t i=0; i<length; ++i) {
        crc ^= (uint16_t)data[i] << 8;
        for (unsigned b=0; b<8; ++b)
            crc=(uint16_t)((crc & 0x8000) ? (crc << 1)^0x1021 : crc << 1);
    }
    return crc;
}

size_t proto_encode(const proto_packet_t *p, uint8_t *wire)
{
    uint8_t raw[PROTO_RAW_MAX];
    if (p->length>PROTO_PAYLOAD_MAX) return 0;
    raw[0]=PROTO_VERSION; raw[1]=p->type; raw[2]=p->channel; raw[3]=p->flags;
    proto_put_u16(raw+4,p->sequence); proto_put_u16(raw+6,p->length);
    memcpy(raw+8,p->payload,p->length);
    proto_put_u16(raw+8+p->length,proto_crc16(raw,8+p->length));
    size_t out=1, code_pos=0;
    uint8_t code=1;
    for (size_t i=0; i<p->length+10U; ++i) {
        if (raw[i]==0) { wire[code_pos]=code; code_pos=out++; code=1; }
        else { wire[out++]=raw[i]; ++code; }
    }
    wire[code_pos]=code;
    wire[out++]=0;
    return out;
}

bool proto_feed(proto_parser_t *s, uint8_t byte, proto_packet_t *p)
{
    if (byte) {
        if (s->discarding) return false;
        if (s->used>=sizeof(s->encoded)) { s->discarding=true; ++s->errors; }
        else s->encoded[s->used++]=byte;
        return false;
    }
    if (s->discarding) { s->used=0; s->discarding=false; return false; }
    if (!s->used) return false;
    uint8_t raw[PROTO_RAW_MAX];
    size_t input=0, output=0, size=s->used;
    s->used=0;
    while (input<size) {
        uint8_t code=s->encoded[input++];
        if (!code || input+code-1>size || output+code-1>sizeof(raw)) goto bad;
        for (unsigned i=1; i<code; ++i) raw[output++]=s->encoded[input++];
        if (code!=255 && input<size) {
            if (output>=sizeof(raw)) goto bad;
            raw[output++]=0;
        }
    }
    if (output<10 || raw[0]!=PROTO_VERSION) goto bad;
    uint16_t length=proto_get_u16(raw+6);
    if (length>PROTO_PAYLOAD_MAX || output!=length+10U ||
        proto_crc16(raw,output-2)!=proto_get_u16(raw+output-2)) goto bad;
    p->type=raw[1]; p->channel=raw[2]; p->flags=raw[3];
    p->sequence=proto_get_u16(raw+4); p->length=length;
    memcpy(p->payload,raw+8,length);
    return true;
bad:
    ++s->errors;
    return false;
}

static const uint8_t dlc_lengths[16]={0,1,2,3,4,5,6,7,8,12,16,20,24,32,48,64};
int proto_length_to_dlc(uint8_t length)
{
    for (int i=0; i<16; ++i) if (dlc_lengths[i]==length) return i;
    return -1;
}
uint8_t proto_dlc_to_length(uint8_t dlc) { return dlc<16 ? dlc_lengths[dlc] : 0; }
