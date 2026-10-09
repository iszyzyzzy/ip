from pathlib import Path
import struct
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from dm_can import ACK, RX, TX_RESULT, TX, FD, BRS, Bridge, Packet, Parser, crc16


class FakeSerial:
    def __init__(self):
        self.data = bytearray()
        self.closed = False
        self.writes = []

    @property
    def in_waiting(self):
        return len(self.data)

    def write(self, data):
        self.writes.append(data)
        if data == b'\0':
            return 1
        p = Packet.decode(data[:-1])
        # 等待发送确认时，另外两路的接收帧仍必须保留下来。
        for ch in (1, 2, 3):
            self.data += Packet(RX, ch, 0, 0, struct.pack('<IIB', 0x100 + ch, 1234, 2) + bytes([ch, 0])).encode()
        if p.type == TX:
            self.data += Packet(TX_RESULT, p.channel, 0, p.sequence, struct.pack('<BI', 0, 2000)).encode()
        self.data += Packet(ACK, p.channel, 0, p.sequence, bytes([p.type, 0])).encode()
        return len(data)

    def read(self, size):
        size = min(size, 3)  # 刻意拆开报文边界。
        chunk = bytes(self.data[:size])
        del self.data[:size]
        return chunk

    def close(self):
        self.closed = True


class HostTests(unittest.TestCase):
    def test_crc_known_vector(self):
        self.assertEqual(crc16(b'123456789'), 0x29B1)

    def test_split_merge_and_recovery(self):
        p = Packet(4, 2, 12, 0x1234, bytes(range(80)))
        wire = p.encode()
        parser = Parser()
        result = []
        for byte in wire:
            result.extend(parser.feed(bytes([byte])))
        self.assertEqual(result, [p])
        self.assertEqual(parser.feed(wire * 3), [p] * 3)
        damaged = bytearray(wire)
        damaged[-2] ^= 1
        self.assertEqual(parser.feed(damaged), [])
        self.assertEqual(parser.feed(b'\x01' * 200 + b'\0' + wire), [p])
        self.assertEqual(parser.errors, 2)

    def test_demux_rx_while_waiting(self):
        serial = FakeSerial()
        with Bridge('fake', transport=serial) as bridge:
            seq = bridge.send(3, 0x321, bytes(range(12)), fd=True, brs=True)
            self.assertEqual(seq, 1)
            self.assertEqual([bridge.recv().channel for _ in range(3)], [1, 2, 3])
        self.assertTrue(serial.closed)

    def test_invalid_tx_rejected_before_io(self):
        serial = FakeSerial()
        bridge = Bridge('fake', transport=serial)
        cases = [dict(channel=0, can_id=1), dict(channel=1, can_id=0x800),
                 dict(channel=1, can_id=1, data=b'123456789'),
                 dict(channel=1, can_id=1, brs=True),
                 dict(channel=1, can_id=1, fd=True, remote_length=8)]
        for case in cases:
            with self.assertRaises(ValueError):
                bridge.send(**case)
        self.assertEqual(len(serial.writes), 1)

    def test_timeout_does_not_retry(self):
        serial = FakeSerial()
        serial.read = lambda _: b''
        bridge = Bridge('fake', timeout=0.001, transport=serial)
        with self.assertRaises(TimeoutError):
            bridge.request(1)
        self.assertEqual(len(serial.writes), 2)

    def test_c_python_interop(self):
        executable = Path(__file__).resolve().parents[1] / 'build' / 'test_protocol.exe'
        self.assertTrue(executable.exists(), '请先运行 tests/run.ps1 编译 C 检查程序')
        lines = subprocess.check_output([str(executable), '--vectors'], text=True).splitlines()
        self.assertEqual(len(lines), 243)
        for line in lines:
            pattern, length, hex_wire = line.split()
            pattern, length = int(pattern), int(length)
            data = bytes(0 if pattern == 0 else 255 if pattern == 1 else (i * 17) & 255 for i in range(length))
            p = Packet(TX, 3, 1 | FD | BRS, 0xA100 + length, data)
            self.assertEqual(p.encode().hex(), hex_wire)
            self.assertEqual(Packet.decode(bytes.fromhex(hex_wire)[:-1]), p)


if __name__ == '__main__':
    unittest.main()
