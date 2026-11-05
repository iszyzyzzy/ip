#!/usr/bin/env python3
"""舵机一次性命令行工具：一次调用下发一组指令，不确认、不限制位置。

与 servo_tool.py 的菜单版本功能对应（查询、改ID、脱力、保存上电位置、
往复摆动、保持），但每次只做一件事，便于脚本化和现场快速验证。
位置按协议原样下发（4位位置单位），不做500～2500之类的机构安全判断。
"""

import argparse
import fcntl
import re
import struct
import subprocess
import sys
import time

import serial

PORTS = {"u2": "/dev/ttyAMA1", "u3": "/dev/ttyAMA2"}
TIOCSRS485 = 0x542F
TIOCGRS485 = 0x542E


class Bus:
    def __init__(self, port, baud):
        self.ser = serial.Serial(port, baud, timeout=0.02, rtscts=False, exclusive=True)
        try:
            # 本板手动切换RTS会丢失最后一位后的正常回包，交给内核自动释放。
            self.ser.reset_input_buffer()
            fcntl.ioctl(self.ser.fileno(), TIOCSRS485, struct.pack("8I", 5, 0, 0, 0, 0, 0, 0, 0))
            actual = struct.unpack("8I", fcntl.ioctl(self.ser.fileno(), TIOCGRS485, bytes(32)))
            if actual[0] & 7 != 5:
                raise RuntimeError("串口未接受低有效发送的自动RTS配置")
        except BaseException:
            self.ser.close()
            raise

    def close(self):
        try:
            fcntl.ioctl(self.ser.fileno(), TIOCSRS485, bytes(32))
            self.ser.rts = False
        finally:
            self.ser.close()

    def send(self, address, body, wait):
        wire = f"#{address:03d}{body}!".encode("ascii")
        self.ser.reset_input_buffer()
        self.ser.write(wire)
        self.ser.flush()
        deadline = time.monotonic() + wait
        data = bytearray()
        while time.monotonic() < deadline:
            data.extend(self.ser.read(512))
        reply = bytes(data).replace(wire, b"").decode("ascii", errors="backslashreplace").strip()
        print(f">>> {wire.decode()}    <<< {reply or '（无回包）'}", flush=True)
        return reply

    def read_position(self, servo_id):
        reply = self.send(servo_id, "PRAD", 0.35)
        match = re.search(rf"#{servo_id:03d}P(\d+)!(?!\w)", reply)
        if not match:
            raise RuntimeError(f"ID {servo_id} 未读到位置回包")
        return int(match.group(1))


def is_error(reply):
    return "PERR" in reply or "PTPERROR" in reply


def acquire(args):
    # 只提示不阻止：串口独占由 TIOCEXCL 和内核RTS配置保证，现场需要时可自行判断。
    active = subprocess.run(["systemctl", "is-active", "--quiet", "robotd.service"], check=False)
    if active.returncode == 0:
        print("提示：robotd.service 正在运行，串口可能被占用。", file=sys.stderr)
    return Bus(PORTS.get(args.port, args.port), args.baud)


def run(args, bus):
    """执行子命令，返回进程退出码。"""
    cmd = args.cmd
    wait = args.wait
    if cmd == "raw":
        return 1 if is_error(bus.send(args.id, args.body, 0.35 if wait is None else wait)) else 0
    if cmd in ("status", "pos"):
        bodies = ["PVER", "PID", "PRAD", "PMOD", "PRDA"] if cmd == "status" else ["PRAD"]
        bad = 0
        for body in bodies:
            bad |= is_error(bus.send(args.id, body, 0.35 if wait is None else wait))
        return 1 if bad else 0
    if cmd == "move":
        body = f"P{args.position:04d}"
        if args.time is not None:
            body += f"T{args.time:04d}"
        if args.speed is not None:
            body += f"V{args.speed:04d}"
        # 位置命令本身可能没有ACK，默认只短暂看一眼有没有错误回包。
        return 1 if is_error(bus.send(args.id, body, 0.1 if wait is None else wait)) else 0
    if cmd == "swing":
        start = bus.read_position(args.id)
        duration = args.time / 1000
        for target in (start + args.delta, start - args.delta, start):
            body = f"P{target:04d}T{args.time:04d}"
            bus.send(args.id, body, 0.05 if wait is None else wait)
            time.sleep(duration)
        return 0
    if cmd == "free":
        return 1 if is_error(bus.send(args.id, "PULK", 0.35 if wait is None else wait)) else 0
    if cmd == "hold":
        return 1 if is_error(bus.send(args.id, "PULR", 0.2 if wait is None else wait)) else 0
    if cmd == "stop":
        return 1 if is_error(bus.send(args.id, "PDST", 0.2 if wait is None else wait)) else 0
    if cmd == "save":
        reply = bus.send(args.id, "PCSD", 0.6 if wait is None else wait)
        if not re.search(rf"#{args.id:03d}P(?:OK)?!|#OK!", reply):
            print("未收到明确成功回包，无法确认已保存（未重发）。", file=sys.stderr)
        return 0
    if cmd == "id":
        bus.send(args.id, f"PID{args.new_id:03d}", 0.6 if wait is None else wait)
        # 直接到新地址读身份，不依据写入ACK猜测结果。
        reply = bus.send(args.new_id, "PVER", 0.35 if wait is None else wait)
        if re.search(rf"#{args.new_id:03d}P[^!]*[Ss][Ee][Rr][Vv][Oo]", reply):
            print(f"新ID {args.new_id} 已读到版本；掉电保持需重新上电确认。")
            return 0
        print(f"新ID {args.new_id} 未验证，请检查原ID与新ID。", file=sys.stderr)
        return 1
    raise SystemExit(f"未知命令 {cmd}")


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--port", default="u2", help="u2、u3 或设备路径（默认u2）")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--id", type=int, default=1, help="目标ID（默认1）")
    parser.add_argument("--wait", type=float, default=None, help="覆盖默认回包等待秒数")
    sub = parser.add_subparsers(dest="cmd", required=True)

    sub.add_parser("status", help="PVER/PID/PRAD/PMOD/PRDA 全量查询")
    sub.add_parser("pos", help="只读位置 PRAD")
    sub.add_parser("free", help="释放扭力 PULK")
    sub.add_parser("hold", help="保持当前位置 PULR")
    sub.add_parser("stop", help="停止当前轨迹 PDST")
    sub.add_parser("save", help="保存当前位置为上电位置 PCSD")

    p_move = sub.add_parser("move", help="走到绝对位置 P####[T####][V####]")
    p_move.add_argument("position", type=int, help="目标位置，4位位置单位")
    p_move.add_argument("--time", type=int, default=None, help="到达时间毫秒 T####")
    p_move.add_argument("--speed", type=int, default=None, help="速度 V####")

    p_swing = sub.add_parser("swing", help="从当前位置往复摆动再回原位")
    p_swing.add_argument("--delta", type=int, default=20, help="相对摆幅，默认20")
    p_swing.add_argument("--time", type=int, default=1000, help="每段毫秒，默认1000")

    p_id = sub.add_parser("id", help="改ID并在新地址验证")
    p_id.add_argument("new_id", type=int)

    p_raw = sub.add_parser("raw", help="原样发送命令体，如 raw P1500T1000")
    p_raw.add_argument("body")

    args = parser.parse_args()
    if not 0 <= args.id <= 255:
        parser.error("ID应为0～255（255为广播）")
    if args.cmd == "id" and not 0 <= args.new_id <= 253:
        parser.error("普通ID应为0～253")
    if args.cmd == "move" and not 0 <= args.position <= 9999:
        parser.error("位置应为0～9999（4位协议字段）")
    if args.cmd == "swing":
        if not 0 <= args.delta <= 9999:
            parser.error("摆幅应为0～9999")
        if not 0 <= args.time <= 9999:
            parser.error("每段时间应为0～9999毫秒")
    if args.cmd == "move":
        for name in ("time", "speed"):
            value = getattr(args, name)
            if value is not None and not 0 <= value <= 9999:
                parser.error(f"{name} 应为0～9999（4位协议字段）")

    bus = acquire(args)
    try:
        return run(args, bus)
    finally:
        bus.close()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyboardInterrupt, EOFError):
        raise SystemExit("\n已中断；退出不改变扭力状态。")
    except (OSError, ValueError, RuntimeError, serial.SerialException) as error:
        raise SystemExit(f"失败：{error}")
