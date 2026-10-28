#!/usr/bin/env python3
"""树莓派单线舵机调试：改 ID、脱力、保存上电位置、小幅摆动。"""

import argparse
import fcntl
import re
import struct
import subprocess
import time

import serial


PORTS = {"u2": "/dev/ttyAMA1", "u3": "/dev/ttyAMA2"}


class Servo:
    def __init__(self, port, baud, servo_id):
        self.id = servo_id
        self.port = serial.Serial(port, baud, timeout=0.02, rtscts=False, exclusive=True)
        try:
            # 手动 flush/RTS 在本板上会丢失正常答复，交给内核处理最后一位。
            settings = struct.pack("8I", 5, 0, 0, 0, 0, 0, 0, 0)
            fcntl.ioctl(self.port.fileno(), 0x542F, settings)
            actual = struct.unpack("8I", fcntl.ioctl(self.port.fileno(), 0x542E, bytes(32)))
            if actual[0] & 7 != 5:
                raise RuntimeError("串口未接受低有效发送的自动 RTS 配置")
        except BaseException:
            self.port.close()
            raise

    def close(self):
        try:
            fcntl.ioctl(self.port.fileno(), 0x542F, bytes(32))
            self.port.rts = False
        finally:
            self.port.close()

    def command(self, body, *, servo_id=None, wait=0.35):
        address = self.id if servo_id is None else servo_id
        wire = f"#{address:03}{body}!".encode("ascii")
        self.port.reset_input_buffer()
        self.port.write(wire)
        self.port.flush()
        end = time.monotonic() + wait
        data = bytearray()
        while time.monotonic() < end:
            data.extend(self.port.read(512))
        reply = bytes(data).replace(wire, b"").decode("ascii", errors="backslashreplace")
        print(f"发送 {wire.decode()}  收到 {reply.strip() or '（无回包）'}")
        if "PERR" in reply or "PTPERROR" in reply:
            print("注意：收到错误回包；写入命令不会自动重发。")
        return reply

    def position(self):
        if self.id == 255:
            raise RuntimeError("先设置普通 ID，再查询位置或摆动")
        reply = self.command("PRAD")
        match = re.search(rf"#{self.id:03}P(\d+)!(?!\w)", reply)
        if not match or not 500 <= int(match[1]) <= 2500:
            raise RuntimeError("没有读到有效位置，已取消后续操作")
        return int(match[1])

    def status(self):
        self.command("PVER")
        self.command("PID")
        self.position()
        self.command("PMOD")
        self.command("PRDA")

    def set_id(self):
        value = int(input("新 ID（0～253）："))
        if not 0 <= value <= 253:
            raise ValueError("普通 ID 必须在 0～253 之间")
        if input("确认当前这条总线只接一颗舵机？输入 yes：").strip() != "yes":
            return
        reply = self.command(f"PID{value:03}", wait=0.6)
        # 不依据写入 ACK 猜测是否成功，直接到新地址读回身份。
        verified = self.command("PVER", servo_id=value)
        if re.search(rf"#{value:03}P[^!]*[Ss][Ee][Rr][Vv][Oo]", verified):
            self.id = value
            print(f"已在新 ID {value} 读到版本。断电保持仍需重新上电后确认。")
        else:
            print(f"新 ID 未验证，未重发写入。请检查原 ID 和新 ID；写入回包：{reply!r}")

    def startup(self):
        position = self.position()
        print(f"当前位置读数 {position}，将保存为上电位置，不改变坐标中值。")
        if input("手动摆好且已停止后，输入 save 保存：").strip() == "save":
            reply = self.command("PCSD", wait=0.6)
            if re.search(rf"#{self.id:03}P(?:OK)?!|#OK!", reply):
                print("设备返回成功；请在方便时重新上电验证启动动作。")
            else:
                print("未收到明确成功回包，不能确认已保存；不会自动重发。")

    def wiggle(self):
        center = self.position()
        text = input("摆幅（位置读数单位，默认20，最大50）：").strip()
        delta = int(text or "20")
        if not 1 <= delta <= 50 or not 500 <= center - delta < center + delta <= 2500:
            raise ValueError("摆幅超范围，或目标超出500～2500")
        print(f"将先恢复扭力，从当前位置 {center} 慢速走到 {center + delta}、{center - delta}，再回 {center}。")
        if input("确认线缆和机构有余量，输入 move 开始：").strip() != "move":
            return
        try:
            # 脱力状态可能持续到下一条恢复指令，位置目标不应隐含承担恢复扭力。
            reply = self.command("PULR", wait=0.15)
            if "PERR" in reply or "PTPERROR" in reply:
                raise RuntimeError("恢复扭力收到错误回包，已取消摆动")
            positions = [center]
            for target in (center + delta, center - delta, center):
                self.command(f"P{target:04}T1000", wait=0.05)
                time.sleep(1.15)
                actual = self.position()
                positions.append(actual)
                print(f"目标 {target}，实际读数 {actual}，差值 {actual - target:+d}。")
            if max(positions) - min(positions) <= 2:
                print("未观察到明显位移。请检查扭力、保护状态、软限位及工作模式；不要继续加大摆幅。")
                self.command("PMOD")
                self.command("PRDA")
            else:
                print(f"本次读数范围 {min(positions)}～{max(positions)}，最后读数 {actual}。")
        except BaseException:
            # 中断后停止继续追赶目标，不突然卸力导致臂体下落。
            self.command("PDST", wait=0.1)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", default="u2", help="u2、u3 或设备路径（默认u2）")
    parser.add_argument("--baud", type=int, default=115200)
    parser.add_argument("--id", type=int, default=1, help="当前ID；未知且单颗接入时可用255改号")
    parser.add_argument("action", nargs="?", choices=("menu", "status"), default="menu")
    args = parser.parse_args()
    if not 0 <= args.id <= 253 and args.id != 255:
        parser.error("ID应为0～253，或广播地址255")
    # TIOCEXCL不能排除已经打开串口的进程，先检查已知的设备所有者。
    active = subprocess.run(
        ["systemctl", "is-active", "--quiet", "robotd.service"], check=False
    )
    if active.returncode == 0:
        parser.exit(1, "请先运行 sudo systemctl stop robotd，再启动本工具。\n")
    device = Servo(PORTS.get(args.port, args.port), args.baud, args.id)
    try:
        print(f"已打开 {device.port.port}，{args.baud}，内核自动RTS；当前ID {device.id}")
        if args.action == "status":
            device.status()
            return
        while True:
            print(f"\n当前ID {device.id}：1查询  2改ID  3脱力  4保存上电位置  5小幅摆动  6保持当前位置  0退出")
            choice = input("> ").strip()
            try:
                if choice == "0":
                    break
                if choice == "1":
                    device.status()
                elif choice == "2":
                    device.set_id()
                elif choice == "3":
                    if input("脱力可能使机械臂下落，请先托住；输入 free：").strip() == "free":
                        device.command("PULK")
                elif choice == "4":
                    device.startup()
                elif choice == "5":
                    device.wiggle()
                elif choice == "6":
                    device.position()
                    device.command("PULR")
            except (ValueError, RuntimeError) as error:
                print(f"操作未完成：{error}")
    finally:
        device.close()


if __name__ == "__main__":
    try:
        main()
    except (KeyboardInterrupt, EOFError):
        print("\n已退出；退出本身不改变舵机扭力状态。")
    except (OSError, serial.SerialException, RuntimeError) as error:
        raise SystemExit(f"调试失败：{error}") from error
