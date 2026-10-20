"""在没有其他DSI命令发送任务时，读取CAM/DISP0状态或设置标准亮度。"""
import argparse
import ctypes
import json
import mmap
import os
import time

# 地址已由本机设备树及控制器VERSION核对；寄存器偏移来自RP1主机源码。
BASE = 0x1F0011C000
GEN_HDR = 0x6C
GEN_PLD_DATA = 0x70
CMD_PKT_STATUS = 0x74


class Dsi:
    def __init__(self):
        self.fd = os.open("/dev/mem", os.O_RDWR | os.O_SYNC)
        self.mem = mmap.mmap(self.fd, 4096, offset=BASE)
        if self.read32(0) != 0x3133312A:
            raise RuntimeError("控制器版本不符，停止访问")

    def read32(self, offset):
        return ctypes.c_uint32.from_buffer(self.mem, offset).value

    def write32(self, offset, value):
        # 总线要求对齐的32位访问，不能用逐字节拷贝替代。
        ctypes.c_uint32.from_buffer(self.mem, offset).value = value

    def wait(self, ready):
        end = time.monotonic() + 0.5
        while not ready():
            if time.monotonic() > end:
                raise RuntimeError(f"DSI等待超时，状态={self.read32(CMD_PKT_STATUS):#x}")
            time.sleep(0.0001)

    def send(self, header):
        self.wait(lambda: self.read32(CMD_PKT_STATUS) & 0xF == 5)
        self.write32(GEN_HDR, header)
        self.wait(lambda: self.read32(CMD_PKT_STATUS) & 0xF == 5)

    def write(self, reg, value):
        self.send(0x15 | reg << 8 | value << 16)

    def read(self, reg):
        self.send(0x137)  # 只请求一字节，避免把FIFO填充当成有效数据。
        self.send(0x06 | reg << 8)
        self.wait(lambda: self.read32(CMD_PKT_STATUS) & 0x50 == 0)
        return self.read32(GEN_PLD_DATA) & 0xFF

    def close(self):
        self.mem.close()
        os.close(self.fd)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--brightness", type=lambda x: int(x, 0))
    parser.add_argument("--disable-test-pattern", action="store_true", help="清除页1的4A自动循环和测试图使能位")
    args = parser.parse_args()
    if args.brightness is not None and not 0 <= args.brightness <= 255:
        parser.error("亮度必须为0～255")
    dsi = Dsi()
    try:
        # 页切换属于易失性访问；先核对内部型号，再允许修改标准亮度。
        dsi.write(0xE0, 1)
        ident = [dsi.read(r) for r in (0x8D, 0x8E, 0x8F)]
        if ident[:2] != [0x93, 0x65]:
            raise RuntimeError(f"JD内部型号不符：{ident}")
        pattern_change = None
        if args.disable_test_pattern:
            before = dsi.read(0x4A)
            dsi.write(0x4A, before & ~0xC0)
            time.sleep(0.05)
            after = dsi.read(0x4A)
            pattern_change = {"修改前": f"{before:02x}", "修改后": f"{after:02x}"}
        dsi.write(0xE0, 0)
        if args.brightness is not None:
            dsi.write(0x51, args.brightness)
            time.sleep(0.05)
        pages = {
            0: [0x0A, 0x0C, 0x0E, 0x0F, 0x52, 0x54, 0x56, 0x80, 0x81, 0x82, 0xE4, 0xE5],
            1: [0x00, 0x01, 0x03, 0x04, *range(0x0A, 0x0F), 0x24, 0x37, *range(0x40, 0x4D), *range(0x55, 0x5D)],
            3: list(range(0x98, 0x9D)),
            4: [0x1E, 0x1F, 0x35, 0x36, 0x37, 0xA3, 0xA4, 0xA5],
        }
        result = {"内部型号": " ".join(f"{x:02x}" for x in ident)}
        if pattern_change is not None:
            result["关闭测试图案"] = pattern_change
        for page, regs in pages.items():
            dsi.write(0xE0, page)
            result[f"页{page}"] = {f"{r:02x}": f"{dsi.read(r):02x}" for r in regs}
        result["主机错误"] = [hex(dsi.read32(0xBC)), hex(dsi.read32(0xC0))]
        print(json.dumps(result, ensure_ascii=False, indent=2))
    finally:
        dsi.write(0xE0, 0)
        dsi.close()


if __name__ == "__main__":
    main()
