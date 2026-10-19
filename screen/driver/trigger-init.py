"""重新初始化指定 DSI framebuffer，供示波器捕获 RST 和低速命令。"""
import argparse
import fcntl
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fb", choices=("/dev/fb0", "/dev/fb1"), default="/dev/fb0")
args = parser.parse_args()
name = Path("/sys/class/graphics", Path(args.fb).name, "name").read_text().strip()
if name != "drm-rp1-dsidrmf":
    raise SystemExit(f"{args.fb} 不是 RP1 DSI framebuffer，停止触发")

with open(args.fb, "r+b", buffering=0) as device:
    print(f"{args.fb}：关闭输出并保持复位，准备捕获下一次 RST 上升沿", flush=True)
    try:
        fcntl.ioctl(device, 0x4611, 4)  # FBIOBLANK / FB_BLANK_POWERDOWN
        time.sleep(0.5)
    finally:
        # 捕获操作被中断时也恢复输出，避免把屏幕留在复位状态。
        fcntl.ioctl(device, 0x4611, 0)  # FB_BLANK_UNBLANK
    print(f"{args.fb}：重新初始化完成，输出已恢复", flush=True)
