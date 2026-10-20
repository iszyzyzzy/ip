"""向双屏 framebuffer 写入不同编号的彩条，便于现场核对端口和颜色。"""
import argparse
import fcntl
import mmap
import struct
from pathlib import Path

glyphs = {
    "0": ["111", "101", "101", "101", "111"],
    "1": ["010", "110", "010", "010", "111"],
}
colors = [0xFFFFFF, 0xFFFF00, 0x00FFFF, 0x00FF00, 0xFF00FF, 0xFF0000, 0x0000FF]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fb", type=int, action="append", choices=(0, 1))
parser.add_argument("--pattern", choices=("bars", "red", "green", "blue", "black", "white", "quadrants"), default="bars")
args = parser.parse_args()
solid = {"red": 0xFF0000, "green": 0x00FF00, "blue": 0x0000FF, "black": 0, "white": 0xFFFFFF}
for index in args.fb if args.fb is not None else range(2):
    sys = Path(f"/sys/class/graphics/fb{index}")
    width, height = map(int, sys.joinpath("virtual_size").read_text().strip().split(","))
    stride = int(sys.joinpath("stride").read_text())
    assert int(sys.joinpath("bits_per_pixel").read_text()) == 32
    frame = bytearray(stride * height)
    for y in range(height):
        for x in range(width):
            color = colors[min(x * 7 // width, 6)]
            if args.pattern in solid:
                color = solid[args.pattern]
            elif args.pattern == "quadrants":
                color = 0xFFFFFF if (x < width // 2) == (y < height // 2) else 0
            elif x < 8 or y < 8 or x >= width - 8 or y >= height - 8:
                color = 0xFFFFFF
            # 黑底大编号使两路即使画面旋转也能区分。
            if args.pattern == "bars" and 200 <= x < 520 and 180 <= y < 540:
                color = 0x101010
                gx, gy = (x - 240) // 80, (y - 210) // 60
                if 0 <= gx < 3 and 0 <= gy < 5 and glyphs[str(index)][gy][gx] == "1":
                    color = 0xFFFFFF
            offset = y * stride + x * 4
            frame[offset:offset + 4] = color.to_bytes(4, "little")
    with open(f"/dev/fb{index}", "r+b", buffering=0) as device:
        # 第二路没有控制台时可能尚未启用；显式 modeset 才会发送其初始化表。
        variable = bytearray(160)
        fcntl.ioctl(device, 0x4600, variable)
        struct.pack_into("I", variable, 84, 0x80)  # FB_ACTIVATE_FORCE
        fcntl.ioctl(device, 0x4601, variable)
        with mmap.mmap(device.fileno(), len(frame)) as memory:
            memory[:] = frame
    print(f"fb{index}: {width}x{height} 图案 {args.pattern} 已写入")
