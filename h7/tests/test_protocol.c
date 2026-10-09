#include "protocol.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static unsigned feed(proto_parser_t *s,const uint8_t *wire,size_t length,proto_packet_t *out)
{
    unsigned count=0;
    for (size_t i=0; i<length; ++i) if (proto_feed(s,wire[i],out)) ++count;
    return count;
}

int main(int argc,char **argv)
{
    assert(proto_crc16((const uint8_t *)"123456789",9)==0x29b1);
    proto_parser_t parser={0};
    proto_packet_t out;
    uint8_t wire[PROTO_WIRE_MAX];
    unsigned vectors=0;
    /* 遍历零字节分布和完整负载长度，覆盖 COBS 分段和 CRC 的互通约束。 */
    for (unsigned pattern=0; pattern<3; ++pattern) {
        for (unsigned length=0; length<=PROTO_PAYLOAD_MAX; ++length) {
            proto_packet_t p={.type=CMD_TX,.channel=3,.flags=FLAG_EXT|FLAG_FD|FLAG_BRS,
                              .sequence=0xa100+length,.length=length};
            for (unsigned i=0; i<length; ++i)
                p.payload[i]=pattern==0 ? 0 : (pattern==1 ? 0xff : (uint8_t)(i*17));
            size_t size=proto_encode(&p,wire);
            assert(size<=sizeof(wire) && wire[size-1]==0);
            assert(feed(&parser,wire,size,&out)==1);
            assert(out.type==p.type && out.channel==p.channel && out.flags==p.flags &&
                   out.sequence==p.sequence && out.length==p.length && !memcmp(p.payload,out.payload,length));
            if (argc==2 && !strcmp(argv[1],"--vectors")) {
                printf("%u %u ",pattern,length);
                for (size_t i=0; i<size; ++i) printf("%02x",wire[i]);
                puts("");
            }
            ++vectors;
        }
    }
    proto_packet_t p={.type=CMD_INFO,.sequence=1};
    size_t size=proto_encode(&p,wire);
    wire[size-2]^=0x01;
    assert(feed(&parser,wire,size,&out)==0 && parser.errors==1);
    uint8_t oversized[PROTO_WIRE_MAX+30];
    memset(oversized,1,sizeof(oversized));
    assert(feed(&parser,oversized,sizeof(oversized),&out)==0);
    assert(!proto_feed(&parser,0,&out));
    size=proto_encode(&p,wire);
    assert(feed(&parser,wire,size,&out)==1);
    uint8_t truncated[]={5,1,2,0};
    assert(feed(&parser,truncated,sizeof(truncated),&out)==0);
    assert(feed(&parser,wire,size,&out)==1);
    for (unsigned i=0; i<16; ++i) assert(proto_length_to_dlc(proto_dlc_to_length(i))==(int)i);
    assert(proto_length_to_dlc(9)==-1 && proto_length_to_dlc(63)==-1);
    if (argc==1) printf("C 协议检查通过：%u 个互通向量，损坏帧与超长帧恢复正常\n",vectors);
    return 0;
}
