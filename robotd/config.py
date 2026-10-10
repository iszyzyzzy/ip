"""配置只在此处定义和校验；设备中的设置是它的运行时副本。"""
from __future__ import annotations

import copy
import ipaddress
import json
import math
from google.protobuf.json_format import ParseDict
from .pb import eyes_pb2 as eye_pb


def number(value, lo, hi, name):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{name} 必须是数字")
    if not math.isfinite(value) or not lo <= value <= hi:
        raise ValueError(f"{name} 必须在 [{lo}, {hi}] 内且为有限数值")


def integer(value, lo, hi, name):
    number(value, lo, hi, name)
    if int(value) != value:
        raise ValueError(f"{name} 必须是整数")


def same_keys(value, template, name):
    if not isinstance(value, dict) or value.keys() != template.keys():
        raise ValueError(f"{name} 字段应为 {', '.join(template)}")


EYE_RANGES = {
    "scale": (0.5, 1.5), "spacing": (0.38, 0.7), "iris_size": (0.1, 0.22),
    "pupil": (0.2, 0.65), "tilt": (-0.2, 0.3), "glow": (0, 1),
    "expression": (-1, 1), "response": (3, 30), "blink_interval": (0.5, 20),
    "blink_duration": (0.08, 1), "blink_variation": (0, 0.8),
    "iris": {"r": (0, 1), "g": (0, 1), "b": (0, 1)},
    "background": {"r": (0, 1), "g": (0, 1), "b": (0, 1)},
    "jelly": {"strength": (0, 1), "frequency": (1, 12), "damping": (0.15, 1.5)},
    "simulator": {
        "target_x": (-1, 1), "target_y": (-1, 1), "motion_amplitude": (0, 0.3),
        "motion_speed": (0.1, 8), "convergence": (0, 0.3), "left_openness": (0, 1.4),
        "right_openness": (0, 1.4), "left_brow": (-1, 1), "right_brow": (-1, 1),
        "roll": (-0.8, 0.8),
    },
    "art": {"half_width": (0.2, 0.36), "upper_height": (0.08, 0.3),
            "lower_height": (0.06, 0.28), "roundness": (0.5, 1.5),
            "iris_aspect": (0.6, 1.2), "brow_height": (0.22, 0.5),
            "lid_texture_height": (0.04, 0.25), "shadow_strength": (0, 0.5)},
}


def default_config():
    tune = dict(position_kp=8.0, velocity_kp=0.12, velocity_ki=0.3,
                max_velocity=3.0, max_output=0.5, max_position=12.5)
    axis = dict(min_rad=-0.5, max_rad=0.5, offset_rad=0.0, direction=1, tuning=tune)
    joint = dict(id=0, min_rad=-0.5, max_rad=0.5, center_pwm=1500,
                 pwm_per_rad=2000 / math.radians(270), direction=1)
    arm = dict(enabled=True, port="/dev/ttyAMA2", baudrate=115200,
               rts_tx=True, direction_mode="manual", timeout_ms=80,
               joints=[dict(joint, id=i) for i in range(4)])
    return {
        "schema_version": 1,
        "metadata": {"name": "robot", "description": ""},
        "h7": {"port": "/dev/serial/by-id/REPLACE_WITH_H7", "timeout_ms": 150},
        "chassis": {"enabled": True, "channel": 2, "bitrate": 500000,
                    "max_speed_m_s": 0.3, "max_yaw_rate_rad_s": 0.5,
                    "max_steering_rad": 0.698, "feedback_timeout_ms": 300},
        "neck": {"enabled": True, "channel": 1, "yaw": copy.deepcopy(axis),
                 "pitch": dict(copy.deepcopy(axis), tuning=dict(tune, position_kp=6.0,
                             velocity_kp=0.25, velocity_ki=0.5))},
        "left_arm": copy.deepcopy(arm),
        "right_arm": dict(copy.deepcopy(arm), port="/dev/ttyAMA3"),
        "eyes": {"enabled": True, "target": "unix:/tmp/oculus-eyes.sock", "settings": {
            "mode": "IDLE", "scale": 1.0, "spacing": 0.48, "iris_size": 0.218,
            "pupil": 0.36, "tilt": 0.045, "glow": 0.0, "expression": 0.0,
            "response": 14.0, "iris": {"r": 1.0, "g": 1.0, "b": 1.0},
            "background": {"r": 1.0, "g": 0.945, "b": 0.94}, "auto_blink": True,
            "mirror_tracking": False, "follow_head_roll": True,
            "show_controls": False, "fullscreen": True,
            "jelly": {"enabled": False, "strength": 0.5, "frequency": 5.0, "damping": 0.7},
            "layers": dict.fromkeys(("iris", "pupil", "highlights", "sclera",
                                     "upper_lid", "lower_lid", "brow"), True),
            "simulator": {"target_x": 0.0, "target_y": 0.0, "eye_motion": False,
                          "motion_amplitude": 0.03, "motion_speed": 1.0,
                          "convergence": 0.0, "left_openness": 1.0, "right_openness": 1.0,
                          "left_brow": 0.0, "right_brow": 0.0, "roll": 0.0,
                          "show_target": False},
            "blink_interval": 3.5, "blink_duration": 0.24, "blink_variation": 0.3,
            "tracking_port": 49983, "phone_ip": "",
            "art": {"half_width": 0.255, "upper_height": 0.205, "lower_height": 0.205,
                    "roundness": 0.85, "iris_aspect": 0.9, "brow_height": 0.3,
                    "lid_texture_height": 0.16, "shadow_strength": 0.12},
        }},
    }


def validate_eye_settings(settings):
    template = default_config()["eyes"]["settings"]

    def check(data, default, ranges, name):
        same_keys(data, default, name)
        for k, v in data.items():
            path = f"{name}.{k}"
            if isinstance(default[k], dict):
                check(v, default[k], ranges.get(k, {}), path)
            elif isinstance(default[k], bool):
                if not isinstance(v, bool):
                    raise ValueError(f"{path} 必须是布尔值")
            elif k in ranges:
                number(v, *ranges[k], path)
    check(settings, template, EYE_RANGES, "eyes.settings")
    integer(settings["tracking_port"], 1024, 65535, "tracking_port")
    if not isinstance(settings["phone_ip"], str):
        raise ValueError("phone_ip 必须是字符串")
    if settings["phone_ip"]:
        ipaddress.IPv4Address(settings["phone_ip"])
    if settings["mode"] not in ("IDLE", "MOUSE", "IFACIALMOCAP", "EXTERNAL", "SIMULATOR"):
        raise ValueError("眼屏模式无效")
    ParseDict(settings, eye_pb.Config())


def validate_config(c):
    # metadata 允许扩展，但仍需是可持久化的标准 JSON，不能绕过有限值校验。
    json.dumps(c, allow_nan=False)
    template = default_config()
    same_keys(c, template, "config")
    if isinstance(c["schema_version"], bool) or c["schema_version"] != 1:
        raise ValueError("配置 schema_version 必须为 1")
    if not isinstance(c["metadata"], dict):
        raise ValueError("metadata 必须为对象")
    for name in ("h7", "chassis", "neck", "left_arm", "right_arm", "eyes"):
        same_keys(c[name], template[name], name)
        if name != "h7" and not isinstance(c[name]["enabled"], bool):
            raise ValueError(f"{name}.enabled 必须是布尔值")
    h7 = c["h7"]
    integer(h7["timeout_ms"], 50, 300, "h7.timeout_ms")
    ch = c["chassis"]
    integer(ch["channel"], 2, 3, "chassis.channel")
    if ch["bitrate"] != 500000:
        raise ValueError("RANGER AIR 要求 500 kbps")
    for key, hi in (("max_speed_m_s", 1.5), ("max_yaw_rate_rad_s", 3.259),
                    ("max_steering_rad", 1.571)):
        number(ch[key], 0.001, hi, key)
    integer(ch["feedback_timeout_ms"], 100, 1000, "feedback_timeout_ms")
    if isinstance(c["neck"]["channel"], bool) or c["neck"]["channel"] != 1:
        raise ValueError("当前 H7 固件的颈部通道固定为 CAN1")
    for name in ("yaw", "pitch"):
        axis = c["neck"][name]
        same_keys(axis, template["neck"][name], name)
        number(axis["min_rad"], -12.5, 12.5, name)
        number(axis["max_rad"], axis["min_rad"] + 0.001, 12.5, name)
        number(axis["offset_rad"], -12.5, 12.5, name)
        if axis["direction"] not in (-1, 1) or isinstance(axis["direction"], bool):
            raise ValueError(f"{name}.direction 必须为 ±1")
        tune = axis["tuning"]
        same_keys(tune, template["neck"][name]["tuning"], "tuning")
        for key, hi in (("position_kp", 100), ("velocity_kp", 100), ("velocity_ki", 100),
                        ("max_velocity", 30), ("max_output", 3 if name == "yaw" else 10),
                        ("max_position", 12.5)):
            number(tune[key], 0 if key.endswith("kp") or key.endswith("ki") else 0.001,
                   hi, f"{name}.{key}")
        if any(abs(axis["direction"] * v + axis["offset_rad"]) > tune["max_position"]
               for v in (axis["min_rad"], axis["max_rad"])):
            raise ValueError(f"{name} 限位换算后超过 H7 max_position")
    ports = [h7["port"]]
    for name in ("left_arm", "right_arm"):
        arm = c[name]
        integer(arm["baudrate"], 9600, 1000000, "baudrate")
        integer(arm["timeout_ms"], 20, 200, "arm.timeout_ms")
        if not isinstance(arm["rts_tx"], bool) or arm["direction_mode"] not in ("manual", "kernel"):
            raise ValueError("RTS 配置无效")
        if not isinstance(arm["joints"], list) or len(arm["joints"]) != 4:
            raise ValueError("每边机械臂需要四个关节")
        ids = []
        for j in arm["joints"]:
            same_keys(j, template[name]["joints"][0], "joint")
            integer(j["id"], 0, 253, "servo.id")
            ids.append(j["id"])
            integer(j["center_pwm"], 500, 2500, "center_pwm")
            number(j["pwm_per_rad"], 1, 10000, "pwm_per_rad")
            number(j["min_rad"], -math.pi, math.pi, "joint.min_rad")
            number(j["max_rad"], j["min_rad"] + 0.001, math.pi, "joint.max_rad")
            if j["direction"] not in (-1, 1) or isinstance(j["direction"], bool):
                raise ValueError("joint.direction 必须为 ±1")
            for angle in (j["min_rad"], j["max_rad"]):
                number(j["center_pwm"] + j["direction"] * angle * j["pwm_per_rad"],
                       500, 2500, "关节限位对应 PWM")
        if len(set(ids)) != 4:
            raise ValueError(f"{name} 同一总线上的舵机 ID 不能重复")
        if arm["enabled"]:
            ports.append(arm["port"])
        elif not isinstance(arm["port"], str) or not arm["port"]:
            raise ValueError("串口名称不能为空")
    if any(not isinstance(p, str) or not p for p in ports) or len(set(ports)) != len(ports):
        raise ValueError("串口必须非空且不能重复")
    if not isinstance(c["eyes"]["target"], str) or not c["eyes"]["target"]:
        raise ValueError("eyes.target 不能为空")
    validate_eye_settings(c["eyes"]["settings"])
