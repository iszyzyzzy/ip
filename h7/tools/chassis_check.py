#!/usr/bin/env python3
"""通过H723 USB CDC采集RANGER AIR底盘反馈；不发送运动或模式切换帧。"""

import argparse
from collections import Counter
import json
import math
from pathlib import Path
import time

from dm_can import Bridge


def decode(can_id, data):
    if can_id == 0x211 and len(data) == 8:
        return {
            "type": "system", "state": data[0], "control_mode": data[1],
            "voltage_v": int.from_bytes(data[2:4], "big") / 10,
            "fault_bits": int.from_bytes(data[4:8], "big"),
        }
    if can_id == 0x221 and len(data) == 8:
        values = [int.from_bytes(data[i:i + 2], "big", signed=True) / 1000 for i in (0, 2, 6)]
        return dict(type="motion", speed_m_s=values[0], yaw_rate_rad_s=values[1], steering_rad=values[2])
    if can_id == 0x291 and len(data) == 3:
        return dict(type="mode", mode=data[0], switching=bool(data[1]), drive_mode=data[2])
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", required=True, help="优先使用 /dev/serial/by-id/... 稳定路径")
    parser.add_argument("--channel", type=int, choices=(1, 2, 3), default=1)
    parser.add_argument("--seconds", type=float, default=15)
    parser.add_argument("--normal", action="store_true", help="正常接收并ACK；默认只听、不发送CAN ACK")
    parser.add_argument("--output", type=Path, default=Path("chassis-check.jsonl"))
    args = parser.parse_args()
    if not math.isfinite(args.seconds) or not 1 <= args.seconds <= 300:
        parser.error("采集时长须为1～300秒")
    counts = Counter()
    recognized = set()
    with args.output.open("w", encoding="utf-8") as report:
        def record(row):
            text = json.dumps(row, ensure_ascii=False)
            print(text, flush=True)
            report.write(text + "\n")
            report.flush()

        with Bridge(args.port) as bridge:
            record(dict(event="board", info=bridge.info()))
            before = bridge.stats(args.channel)
            record(dict(event="before", stats=before))
            try:
                bridge.configure(args.channel, nominal=500000, listen=not args.normal)
                bridge.frames.clear()
                end = time.monotonic() + args.seconds
                while time.monotonic() < end:
                    frame = bridge.recv(timeout=min(0.1, max(0, end - time.monotonic())))
                    if frame is None or frame.channel != args.channel:
                        continue
                    counts[frame.can_id] += 1
                    row = dict(event="frame", **frame.as_dict())
                    if frame.flags == 0:
                        row["decoded"] = decode(frame.can_id, frame.data)
                        if row["decoded"] is not None:
                            recognized.add(frame.can_id)
                    record(row)
                record(dict(event="after", stats=bridge.stats(args.channel)))
            finally:
                # 诊断会暂改CAN速率/模式，退出时恢复原配置及使能状态。
                mode = before["mode"]
                bridge.configure(
                    args.channel, before["nominal_bitrate"], before["data_bitrate"],
                    bool(mode & 1), mode in (2, 3), mode in (4, 5),
                )
                if not before["enabled"]:
                    bridge.state(args.channel, False)
            record(dict(
                event="summary", counts={hex(key): value for key, value in counts.items()},
                host_rx_dropped=bridge.host_rx_dropped, parser_errors=bridge.parser.errors,
                ranger_feedback=bool(recognized),
            ))
    return 0 if recognized else 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit("已停止采集并尝试恢复CAN配置；没有发送运动指令。")
    except (OSError, ValueError, TimeoutError) as error:
        raise SystemExit(f"底盘检查失败：{error}") from error
