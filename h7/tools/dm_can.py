"""达妙 H723 三路 USB CDC CAN 接口；也可直接导入 Bridge 类构建上位机。"""
from __future__ import annotations

import argparse
from collections import deque
from dataclasses import dataclass
import json
import struct
import sys
import time

VERSION = 1
MAX_PAYLOAD = 80
MAX_WIRE = 92
INFO, CONFIG, STATE, TX, STATS, PING = range(1, 7)
ACK, RX, TX_RESULT = 0x80, 0x81, 0x82
EXT, RTR, FD, BRS, ESI = 1, 2, 4, 8, 16
STATUS_NAMES = ('成功', '参数无效', '队列忙或通道未关闭', '通道关闭或只听',
                'HAL 错误', '发送超时', '已取消', 'bus-off')
NOMINAL_RATES = (50000, 100000, 125000, 250000, 500000, 800000, 1000000)
DATA_RATES = (1000000, 2000000, 3000000, 4000000, 5000000, 6000000, 8000000)
DLC_LENGTHS = (0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 16, 20, 24, 32, 48, 64)
STAT_NAMES = ('nominal_bitrate', 'data_bitrate', 'mode', 'enabled', 'bus_off',
              'tx_error_counter', 'rx_error_counter', 'rx_frames', 'rx_dropped',
              'hardware_rx_loss_events', 'tx_queued', 'tx_success', 'tx_failed',
              'bus_off_events', 'tx_event_loss_events', 'protocol_errors',
              'control_dropped', 'usb_rx_overflow')


def crc16(data: bytes) -> int:
    crc = 0xFFFF
    for byte in data:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ (0x1021 if crc & 0x8000 else 0)) & 0xFFFF
    return crc


def cobs_encode(data: bytes) -> bytes:
    output = bytearray(b'\0')
    position, code = 0, 1
    for byte in data:
        if byte == 0:
            output[position] = code
            position = len(output)
            output.append(0)
            code = 1
        else:
            output.append(byte)
            code += 1
            if code == 255:
                output[position] = code
                position = len(output)
                output.append(0)
                code = 1
    output[position] = code
    return bytes(output)


def cobs_decode(data: bytes) -> bytes:
    output = bytearray()
    position = 0
    while position < len(data):
        code = data[position]
        position += 1
        if not code or position + code - 1 > len(data):
            raise ValueError('COBS 长度无效')
        output.extend(data[position:position + code - 1])
        position += code - 1
        if code != 255 and position < len(data):
            output.append(0)
    return bytes(output)


@dataclass(frozen=True)
class Packet:
    type: int
    channel: int = 0
    flags: int = 0
    sequence: int = 0
    payload: bytes = b''

    def encode(self) -> bytes:
        if len(self.payload) > MAX_PAYLOAD:
            raise ValueError('负载超过 80 字节')
        raw = struct.pack('<BBBBHH', VERSION, self.type, self.channel, self.flags,
                          self.sequence, len(self.payload)) + self.payload
        return cobs_encode(raw + struct.pack('<H', crc16(raw))) + b'\0'

    @classmethod
    def decode(cls, encoded: bytes) -> Packet:
        raw = cobs_decode(encoded)
        if len(raw) < 10:
            raise ValueError('报文过短')
        version, kind, channel, flags, seq, length = struct.unpack_from('<BBBBHH', raw)
        if version != VERSION or length > MAX_PAYLOAD or len(raw) != length + 10:
            raise ValueError('版本或长度不匹配')
        if crc16(raw[:-2]) != struct.unpack_from('<H', raw, len(raw) - 2)[0]:
            raise ValueError('CRC 错误')
        return cls(kind, channel, flags, seq, raw[8:-2])


class Parser:
    def __init__(self):
        self.buffer = bytearray()
        self.discarding = False
        self.errors = 0

    def feed(self, data: bytes) -> list[Packet]:
        result = []
        for byte in data:
            if byte:
                if self.discarding:
                    continue
                if len(self.buffer) >= MAX_WIRE:
                    self.errors += 1
                    self.discarding = True
                else:
                    self.buffer.append(byte)
            else:
                if self.buffer and not self.discarding:
                    try:
                        result.append(Packet.decode(bytes(self.buffer)))
                    except ValueError:
                        self.errors += 1
                self.buffer.clear()
                self.discarding = False
        return result


@dataclass(frozen=True)
class Frame:
    channel: int
    can_id: int
    data: bytes
    flags: int
    timestamp_us: int
    dlc_length: int

    @classmethod
    def from_packet(cls, packet: Packet) -> Frame:
        if len(packet.payload) < 9:
            raise ValueError('接收帧过短')
        can_id, timestamp, length = struct.unpack_from('<IIB', packet.payload)
        expected = 9 + (0 if packet.flags & RTR else length)
        if length not in DLC_LENGTHS or len(packet.payload) != expected:
            raise ValueError('接收帧数据长度错误')
        return cls(packet.channel, can_id, packet.payload[9:], packet.flags, timestamp, length)

    def as_dict(self) -> dict:
        return {'channel': self.channel, 'id': hex(self.can_id), 'data': self.data.hex(),
                'flags': self.flags, 'timestamp_us': self.timestamp_us, 'length': self.dlc_length}


class Bridge:
    """单线程同步接口，收帧会在等待命令响应期间缓存，不会混淆三路通道。"""
    def __init__(self, port: str, timeout: float = 1.0, transport=None):
        if transport is None:
            import serial
            transport = serial.Serial(port, 115200, timeout=0.02, write_timeout=timeout)
            transport.dtr = True
        self.serial = transport
        self.timeout = timeout
        self.parser = Parser()
        self.sequence = 0
        self.inbox = deque()
        self.frames = deque(maxlen=8192)
        self.host_rx_dropped = 0
        self.serial.write(b'\0')

    def close(self):
        self.serial.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def _poll(self):
        data = self.serial.read(max(1, min(getattr(self.serial, 'in_waiting', 0), 4096)))
        for packet in self.parser.feed(data):
            if packet.type == RX:
                if len(self.frames) == self.frames.maxlen:
                    self.host_rx_dropped += 1
                self.frames.append(Frame.from_packet(packet))
            else:
                self.inbox.append(packet)
                # 对迟到、无人等待的控制报文设置上限，长时间监控不会持续增长内存。
                if len(self.inbox) > 256:
                    self.inbox.popleft()

    def _wait(self, kind: int, seq: int, channel: int, timeout: float | None = None) -> Packet:
        deadline = time.monotonic() + (self.timeout if timeout is None else timeout)
        while True:
            for packet in self.inbox:
                if packet.type == kind and packet.sequence == seq and packet.channel == channel:
                    self.inbox.remove(packet)
                    return packet
            if time.monotonic() >= deadline:
                raise TimeoutError(f'等待响应超时：type={kind:#x}, channel={channel}, sequence={seq}；未自动重发')
            self._poll()

    def request(self, kind: int, channel: int = 0, payload: bytes = b'', flags: int = 0) -> tuple[int, bytes]:
        self.sequence = (self.sequence + 1) & 0xFFFF
        sequence = self.sequence
        wire = Packet(kind, channel, flags, sequence, payload).encode()
        if self.serial.write(wire) != len(wire):
            raise IOError('串口写入不完整')
        response = self._wait(ACK, sequence, channel)
        if len(response.payload) < 2 or response.payload[0] != kind:
            raise IOError('命令响应不匹配')
        self._check_status(response.payload[1])
        return sequence, response.payload[2:]

    @staticmethod
    def _check_status(status: int):
        if status:
            message = STATUS_NAMES[status] if status < len(STATUS_NAMES) else f'未知状态 {status}'
            raise IOError(message)

    def info(self) -> dict:
        _, data = self.request(INFO)
        if len(data) < 8:
            raise IOError('设备信息长度错误')
        return {'firmware': '.'.join(map(str, data[:3])), 'channels': data[3],
                'max_data_length': data[4], 'tx_slots_per_channel': data[5],
                'capabilities': struct.unpack_from('<H', data, 6)[0], 'board': data[8:].decode('ascii')}

    def state(self, channel: int, enabled: bool):
        self.request(STATE, channel, bytes([bool(enabled)]))

    def configure(self, channel: int, nominal: int = 1000000, data_rate: int = 2000000,
                  fd: bool = False, listen: bool = False, loopback: bool = False):
        if channel not in (1, 2, 3) or nominal not in NOMINAL_RATES or data_rate not in DATA_RATES:
            raise ValueError('通道或速率不支持')
        if listen and loopback:
            raise ValueError('只听和回环不能同时启用')
        mode = int(fd) + (2 if listen else 4 if loopback else 0)
        self.state(channel, False)
        self.request(CONFIG, channel, struct.pack('<IIB', nominal, data_rate, mode))
        self.state(channel, True)

    def stats(self, channel: int) -> dict:
        _, data = self.request(STATS, channel)
        if len(data) != 4 * len(STAT_NAMES):
            raise IOError('统计响应长度错误')
        return dict(zip(STAT_NAMES, struct.unpack('<' + 'I' * len(STAT_NAMES), data)))

    def send(self, channel: int, can_id: int, data: bytes = b'', *, extended: bool = False,
             fd: bool = False, brs: bool = False, remote_length: int | None = None,
             wait: bool = True) -> int:
        length = len(data) if remote_length is None else remote_length
        if channel not in (1, 2, 3) or not 0 <= can_id <= (0x1FFFFFFF if extended else 0x7FF):
            raise ValueError('通道或 CAN ID 无效')
        if length not in DLC_LENGTHS or (not fd and length > 8) or (brs and not fd):
            raise ValueError('数据长度或 BRS 标志无效')
        if remote_length is not None and (fd or data):
            raise ValueError('远程帧不携带数据且只能用于经典 CAN')
        flags = (EXT if extended else 0) | (FD if fd else 0) | (BRS if brs else 0)
        if remote_length is not None:
            flags |= RTR
        seq, _ = self.request(TX, channel, struct.pack('<IB', can_id, length) + data, flags)
        if wait:
            result = self._wait(TX_RESULT, seq, channel)
            if len(result.payload) != 5:
                raise IOError('发送结果长度错误')
            self._check_status(result.payload[0])
        return seq

    def recv(self, timeout: float | None = None) -> Frame | None:
        deadline = time.monotonic() + (self.timeout if timeout is None else timeout)
        while not self.frames:
            if time.monotonic() >= deadline:
                return None
            self._poll()
        return self.frames.popleft()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', help='Windows COM 口，例如 COM7')
    commands = parser.add_subparsers(dest='command', required=True)
    commands.add_parser('ports', help='列出 USB 串口')
    commands.add_parser('info', help='读取固件信息')
    stat = commands.add_parser('stats', help='读取三路统计')
    stat.add_argument('--channel', type=int, choices=(1, 2, 3))
    config = commands.add_parser('config', help='设置通道，配置完成后自动打开')
    config.add_argument('--channel', type=int, choices=(1, 2, 3), required=True)
    config.add_argument('--bitrate', type=int, choices=NOMINAL_RATES, default=1000000)
    config.add_argument('--data-bitrate', type=int, choices=DATA_RATES, default=2000000)
    config.add_argument('--fd', action='store_true')
    modes = config.add_mutually_exclusive_group()
    modes.add_argument('--listen', action='store_true')
    modes.add_argument('--loopback', action='store_true')
    send = commands.add_parser('send', help='发送并等待 CAN 总线完成确认')
    send.add_argument('--channel', type=int, choices=(1, 2, 3), required=True)
    send.add_argument('--id', type=lambda v: int(v, 0), required=True)
    send.add_argument('--data', default='', help='十六进制，例如 01020304')
    send.add_argument('--extended', action='store_true')
    send.add_argument('--fd', action='store_true')
    send.add_argument('--brs', action='store_true')
    send.add_argument('--remote-length', type=int)
    monitor = commands.add_parser('monitor', help='持续显示全部三路接收帧')
    monitor.add_argument('--seconds', type=float, default=0)
    loop = commands.add_parser('loopback', help='三路内部回环检查')
    loop.add_argument('--fd', action='store_true')
    args = parser.parse_args()
    if args.command == 'ports':
        from serial.tools import list_ports
        for port in list_ports.comports():
            print(f'{port.device}: {port.description} ({port.hwid})')
        return
    if not args.port:
        parser.error('此命令需要 --port COMx')
    with Bridge(args.port) as bridge:
        if args.command == 'info':
            print(json.dumps(bridge.info(), ensure_ascii=False, indent=2))
        elif args.command == 'stats':
            print(json.dumps({ch: bridge.stats(ch) for ch in ([args.channel] if args.channel else (1, 2, 3))}, indent=2))
        elif args.command == 'config':
            bridge.configure(args.channel, args.bitrate, args.data_bitrate, args.fd, args.listen, args.loopback)
            print('配置成功')
        elif args.command == 'send':
            seq = bridge.send(args.channel, args.id, bytes.fromhex(args.data), extended=args.extended,
                              fd=args.fd, brs=args.brs, remote_length=args.remote_length)
            print(f'发送成功，sequence={seq}')
        elif args.command == 'monitor':
            deadline = time.monotonic() + args.seconds if args.seconds else float('inf')
            try:
                while time.monotonic() < deadline:
                    frame = bridge.recv(0.1)
                    if frame:
                        print(json.dumps(frame.as_dict()), flush=True)
            except KeyboardInterrupt:
                pass
            print(f'上位机缓存丢帧：{bridge.host_rx_dropped}，协议错误：{bridge.parser.errors}')
        elif args.command == 'loopback':
            previous = {ch: bridge.stats(ch) for ch in (1, 2, 3)}
            try:
                for ch in (1, 2, 3):
                    bridge.configure(ch, fd=args.fd, loopback=True)
                bridge.frames.clear()
                for ch in (1, 2, 3):
                    for length in ((0, 8, 12, 64) if args.fd else (0, 8)):
                        data = bytes((i + ch) & 255 for i in range(length))
                        bridge.send(ch, 0x120 + ch, data, fd=args.fd, brs=args.fd)
                        frame = bridge.recv()
                        if frame is None or (frame.channel, frame.can_id, frame.data) != (ch, 0x120 + ch, data):
                            raise IOError(f'CAN{ch} 回环结果不匹配')
                print('三路内部回环通过')
            finally:
                for ch, state in previous.items():
                    mode = state['mode']
                    bridge.configure(ch, state['nominal_bitrate'], state['data_bitrate'],
                                     bool(mode & 1), mode in (2, 3), mode in (4, 5))
                    if not state['enabled']:
                        bridge.state(ch, False)


if __name__ == '__main__':
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8')
        sys.stderr.reconfigure(encoding='utf-8')
    try:
        main()
    except (OSError, ValueError, TimeoutError) as exc:
        raise SystemExit(f'错误：{exc}') from exc
