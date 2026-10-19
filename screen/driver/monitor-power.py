"""记录启动及供电状态；不自动重启设备，也不重放显示控制。"""
import json
from pathlib import Path
import re
import signal
import subprocess
import time


def command(*args):
    result = subprocess.run(args, capture_output=True, text=True, timeout=3)
    return result.stdout.strip() if result.returncode == 0 else result.stderr.strip()


def emit(event, **data):
    print(json.dumps({"事件": event, **data}, ensure_ascii=False), flush=True)


running = True


def stop(signum, frame):
    global running
    running = False


signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
boot_id = Path("/proc/sys/kernel/random/boot_id").read_text().strip()
power = Path("/proc/device-tree/chosen/power")
power_state = {
    name: power.joinpath(name).read_bytes().hex()
    for name in ("power_reset", "reset_event", "sd_overcurrent", "usb_over_current_detected")
    if power.joinpath(name).exists()
}
emit("监测启动", boot_id=boot_id, reset=command("vcgencmd", "get_rsts"), power=power_state)
last_flags = None
window = []
last_report = 0
while running:
    try:
        adc = command("vcgencmd", "pmic_read_adc")
        match = re.search(r"EXT5V_V\s+volt\(\d+\)=([0-9.]+)V", adc)
        voltage = float(match.group(1)) if match else None
        if voltage is not None:
            window.append(voltage)
        flags = command("vcgencmd", "get_throttled")
        now = time.monotonic()
        # 保留 2 秒采样的区间极值，减少日志写入，又不隐藏已采到的压降。
        if flags != last_flags or now - last_report >= 30:
            connectors = {
                path.parent.name: path.read_text().strip()
                for path in Path("/sys/class/drm").glob("card*-DSI*/enabled")
            }
            emit(
                "供电状态", boot_id=boot_id,
                uptime_s=float(Path("/proc/uptime").read_text().split()[0]),
                throttled=flags, ext5v=voltage,
                ext5v_min=min(window) if window else None,
                ext5v_max=max(window) if window else None,
                soc_temp=command("vcgencmd", "measure_temp"),
                pmic_temp=command("vcgencmd", "measure_temp", "pmic"),
                reset_gpio=command("pinctrl", "get", "34,46"),
                displays=connectors,
            )
            window.clear()
            last_flags, last_report = flags, now
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        emit("采样失败", error=str(error))
    time.sleep(2)
emit("监测停止", boot_id=boot_id)
