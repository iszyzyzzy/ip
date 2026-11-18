"""树莓派本机的微软布局手柄，共用网页网关的 robotd 租约。"""
from __future__ import annotations

import asyncio
from contextlib import suppress
import json
import logging
import os
from pathlib import Path
import struct
import sys
import time

from . import RemoteError


BUTTONS = {0x130: "A", 0x131: "B", 0x133: "X", 0x134: "Y", 0x136: "LB", 0x137: "RB",
           0x13a: "View", 0x13b: "Menu", 0x13d: "L3", 0x13e: "R3"}
MODIFIERS = ("RB", "L3", "R3")
FACES = ("A", "B", "X", "Y")
CHORDS = tuple(f"{mod}+{button}" for mod in MODIFIERS for button in FACES)
EVENT = struct.Struct("=IhBB")
log = logging.getLogger(__name__)


class Joystick:
    def __init__(self, path):
        import fcntl

        device = Path("/sys/class/input", path.name, "device")
        if (device / "id/bustype").read_text().strip() != "0005":
            raise ValueError("非蓝牙手柄")
        address = os.environ.get("ROBOT_REMOTE_GAMEPAD_MAC", "").strip().lower()
        if address and (device / "uniq").read_text().strip().lower() != address:
            raise ValueError("手柄地址不匹配")
        self.file = path.open("rb", buffering=0)
        try:
            os.set_blocking(self.file.fileno(), False)

            def mapping(code, size):
                return fcntl.ioctl(self.file, 0x80006a00 | size << 16 | code, bytes(size))

            self.axes = mapping(0x32, 64)[:mapping(0x11, 1)[0]]
            self.buttons = struct.unpack("=512H", mapping(0x34, 1024))[:mapping(0x12, 1)[0]]
            if not set(BUTTONS).issubset(self.buttons) or not {0, 1}.issubset(self.axes):
                raise ValueError("设备不具有微软手柄布局")
            self.name = mapping(0x13, 128).split(b"\0")[0].decode(errors="replace")
            self.held = set()
            self.x = self.y = 0.
        except BaseException:
            self.file.close()
            raise

    def read(self):
        pressed = []
        while True:
            data = self.file.read(EVENT.size * 64)
            if data is None:
                return pressed
            if not data:
                raise OSError("手柄已断开")
            for _, value, kind, index in EVENT.iter_unpack(data):
                # 重连时内核会重放所有键的初始状态，不能据此取得控制或触发动作。
                initial = bool(kind & 0x80)
                if kind & 0x7f == 1:
                    key = BUTTONS.get(self.buttons[index])
                    if key and value:
                        if not initial and key not in self.held:
                            pressed.append((key, self.held | {key}))
                        self.held.add(key)
                    else:
                        self.held.discard(key)
                elif kind & 0x7f == 2:
                    axis = self.axes[index]
                    v = max(-1., value / 32767)
                    v = 0. if abs(v) < .12 else v
                    if axis == 0:
                        self.x = v
                    elif axis == 1:
                        self.y = v


class Gamepad:
    def __init__(self, gateway, path):
        self.gateway = gateway
        self.path = Path(path)
        self.bindings = {}
        self.error = ""
        if self.path.exists():
            try:
                self.bindings = self.validate(json.loads(self.path.read_text(encoding="utf-8")))
            except (OSError, ValueError) as error:
                self.error = f"读取手柄绑定失败：{error}"
        self.device = None
        self.session = ""
        self.drive = False
        self.renew_at = 0.
        self.chassis = {}

    @staticmethod
    def validate(bindings):
        if not isinstance(bindings, dict) or any(
            key not in CHORDS or not isinstance(name, str) or not name.strip() or len(name) > 64
            for key, name in bindings.items()
        ):
            raise ValueError("手柄组合键或动作名无效")
        return bindings.copy()

    def save(self, bindings):
        bindings = self.validate(bindings)
        self.path.parent.mkdir(parents=True, exist_ok=True)
        temp = self.path.with_suffix(".tmp")
        temp.write_text(json.dumps(bindings, ensure_ascii=False, indent=2), encoding="utf-8")
        temp.replace(self.path)
        self.bindings = bindings

    def status(self):
        return {"device": self.device.name if self.device else "", "error": self.error,
                "controlling": bool(self.session), "bindings": self.bindings.copy(),
                "held": sorted(self.device.held) if self.device else []}

    async def release(self):
        session, self.session = self.session, ""
        self.drive = False
        if session:
            with suppress(RemoteError):
                await self.gateway.execute("release", {"session_id": session})

    async def tick(self, pad, pressed):
        execute = self.gateway.execute
        if any(key == "View" for key, _ in pressed):
            await execute("stop", {"latch": True})
            self.session = ""
            self.drive = False
            return
        if any(key == "Menu" for key, _ in pressed):
            if self.session:
                await self.release()
            elif not pad.x and not pad.y and not (pad.held - {"Menu"}):
                self.chassis = (await execute("get_config", {}))["config"]["chassis"]
                lease = await execute("acquire", {"owner": "蓝牙手柄", "ttl_ms": 1000})
                self.session = lease["session_id"]
                self.renew_at = time.monotonic() + .25
                self.error = ""
            else:
                self.error = "请松开其他按键并将摇杆回中，再按 Menu"
            return
        if not self.session:
            return
        params = {"session_id": self.session}
        if time.monotonic() >= self.renew_at:
            await execute("renew", params)
            self.renew_at = time.monotonic() + .25
        if "LB" in pad.held:
            if self.chassis.get("enabled"):
                if not self.drive and not any(key == "LB" for key, _ in pressed):
                    return
                speed = -pad.y * min(.3, self.chassis["max_speed_m_s"])
                steer = -pad.x * min(.698, self.chassis["max_steering_rad"])
                await execute("targets", {**params, "targets": {"chassis": {
                    "mode": "ACKERMANN", "speed_m_s": speed, "steering_rad": steer, "ttl_ms": 200,
                }}})
                self.drive = True
            else:
                self.error = "底盘未启用"
            return
        if self.drive:
            self.drive = False
            await execute("targets", {**params, "targets": {"chassis": {"mode": "PARK", "ttl_ms": 200}}})
            return
        for key, held in pressed:
            if key in FACES and len(held) == 2:
                modifier = next(iter(held - {key}))
                action = self.bindings.get(f"{modifier}+{key}")
                if action:
                    await execute("run_action", {**params, "name": action})
                    self.error = ""
                    return

    async def run(self):
        if sys.platform != "linux":
            return
        while True:
            try:
                for path in sorted(Path("/dev/input").glob("js[0-9]*")):
                    try:
                        self.device = Joystick(path)
                        break
                    except ValueError:
                        continue
                if self.device:
                    self.error = ""
                    log.info("已连接手柄：%s", self.device.name)
                    while True:
                        try:
                            await self.tick(self.device, self.device.read())
                        except RemoteError as error:
                            self.error = str(error)
                            await self.release()
                        await asyncio.sleep(.05)
            except OSError as error:
                self.error = f"手柄接口：{error}"
            finally:
                if self.device:
                    self.device.file.close()
                    self.device = None
                await self.release()
            await asyncio.sleep(1)
