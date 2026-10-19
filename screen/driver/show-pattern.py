"""向双屏 framebuffer 写入不同编号的彩条，便于现场核对端口和颜色。"""
import mmap
from pathlib import Path

glyphs = {
    "0": ["111", "101", "101", "101", "111"],
    "1": ["010", "110", "010", "010", "111"],
}
colors = [0xFFFFFF, 0xFFFF00, 0x00FFFF, 0x00FF00, 0xFF00FF, 0xFF0000, 0x0000FF]
for index in range(2):
    sys = Path(f"/sys/class/graphics/fb{index}")
    width, height = map(int, sys.joinpath("virtual_size").read_text().strip().split(","))
    stride = int(sys.joinpath("stride").read_text())
    assert int(sys.joinpath("bits_per_pixel").read_text()) == 32
    frame = bytearray(stride * height)
    for y in range(height):
        for x in range(width):
            color = colors[min(x * 7 // width, 6)]
            if x < 8 or y < 8 or x >= width - 8 or y >= height - 8:
                color = 0xFFFFFF
            # 黑底大编号使两路即使画面旋转也能区分。
            if 200 <= x < 520 and 180 <= y < 540:
                color = 0x101010
                gx, gy = (x - 240) // 80, (y - 210) // 60
                if 0 <= gx < 3 and 0 <= gy < 5 and glyphs[str(index)][gy][gx] == "1":
                    color = 0xFFFFFF
            offset = y * stride + x * 4
            frame[offset:offset + 4] = color.to_bytes(4, "little")
    with open(f"/dev/fb{index}", "r+b", buffering=0) as device:
        with mmap.mmap(device.fileno(), len(frame)) as memory:
            memory[:] = frame
    print(f"fb{index}: {width}x{height} 彩条和编号 {index} 已写入")
