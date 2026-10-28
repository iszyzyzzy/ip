#!/usr/bin/env python3
"""双臂八颗舵机相对当前位置增加200单位，再返回起点。默认只预览。"""

import argparse
from contextlib import ExitStack
import subprocess
import time

from servo_tool import PORTS, Servo


def read_positions(buses, ids):
    positions = {}
    for name, bus in buses.items():
        for servo_id in ids:
            bus.id = servo_id
            print(f"[{name} ID {servo_id}]", flush=True)
            positions[name, servo_id] = bus.position()
    return positions


def move_all(buses, positions, duration):
    # 每颗都只发送一次；多舵机总线不使用广播，也不重试运动命令。
    for (name, servo_id), position in positions.items():
        reply = buses[name].command(
            f"P{position:04}T{duration:04}", servo_id=servo_id, wait=0.03
        )
        if "PERR" in reply or "PTPERROR" in reply:
            raise RuntimeError(f"{name} ID {servo_id} 收到错误回包")
    time.sleep(duration / 1000 + 0.5)


def report(label, actual, expected):
    ok = True
    print(f"\n{label}：")
    for (name, servo_id), target in expected.items():
        value = actual[name, servo_id]
        print(f"{name} ID {servo_id}：目标 {target}，实际 {value}，差值 {value - target:+d}")
        ok &= abs(value - target) <= 10
    return ok


def roundtrip(buses, ids, offset, duration, run):
    starts = read_positions(buses, ids)
    targets = {key: position + offset for key, position in starts.items()}
    for (name, servo_id), target in targets.items():
        if not 500 <= target <= 2500:
            raise ValueError(f"{name} ID {servo_id} 目标 {target} 超出500～2500，整组取消")
    print("\n计划（读数单位，不是角度）：")
    for (name, servo_id), start in starts.items():
        print(f"{name} ID {servo_id}：{start} → {targets[name, servo_id]} → {start}")
    if not run:
        print("仅预览，未发送任何动作；确认计划后加 --run 执行。")
        return True
    try:
        move_all(buses, targets, duration)
        outbound = report("去程回读", read_positions(buses, ids), targets)
        time.sleep(0.5)
        move_all(buses, starts, duration)
        returned = report("回程回读", read_positions(buses, ids), starts)
        if outbound and returned:
            print("两段回读均在目标±10单位内。")
        else:
            print("存在未跟随目标的舵机，请查看差值；没有自动追加动作。")
        return outbound and returned
    except BaseException:
        # 中断或通信失败后停住，不自动发起回程或突然卸力。
        for (name, servo_id) in starts:
            try:
                buses[name].command("PDST", servo_id=servo_id, wait=0.05)
            except OSError as error:
                print(f"{name} ID {servo_id} 停止命令发送失败：{error}")
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ports", nargs="+", choices=PORTS, default=["u2", "u3"])
    parser.add_argument("--ids", nargs="+", type=int, default=[1, 2, 3, 4])
    parser.add_argument("--offset", type=int, default=200, help="相对偏移，默认+200单位")
    parser.add_argument("--duration", type=int, default=3000, help="每段到达时间，默认3000毫秒")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--run", action="store_true", help="执行往返；不加时只读位置并预览")
    args = parser.parse_args()
    if len(set(args.ports)) != len(args.ports) or len(set(args.ids)) != len(args.ids):
        parser.error("串口及ID不能重复")
    if not all(0 <= value <= 253 for value in args.ids):
        parser.error("只接受普通ID 0～253")
    if not 1 <= abs(args.offset) <= 200 or not 1000 <= args.duration <= 9999:
        parser.error("偏移绝对值应为1～200，每段时间应为1000～9999毫秒")
    if subprocess.run(
        ["systemctl", "is-active", "--quiet", "robotd.service"], check=False
    ).returncode == 0:
        parser.exit(1, "请先执行 sudo systemctl stop robotd，并退出其他舵机工具。\n")
    with ExitStack() as stack:
        buses = {}
        for name in args.ports:
            buses[name] = Servo(PORTS[name], args.baud, args.ids[0])
            stack.callback(buses[name].close)
        ok = roundtrip(buses, args.ids, args.offset, args.duration, args.run)
        return 0 if ok else 2


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyboardInterrupt, EOFError):
        raise SystemExit("\n已中断，已尝试停止全部轨迹；未自动脱力或发起回程。")
    except (OSError, ValueError, RuntimeError) as error:
        raise SystemExit(f"往返未完成：{error}") from error
