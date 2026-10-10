"""整条指令先校验再发送，避免参数错误造成部分设备已经运动。"""
import math
import re
import struct
from .config import number, integer
from .drivers import arm_pwm


def f32(value):
    return struct.unpack("f", struct.pack("f", value))[0]


def validate_targets(t, c):
    if not t.ListFields():
        raise ValueError("目标不能为空")
    for field in ("chassis", "neck", "left_arm", "right_arm", "eyes", "blink"):
        device = "eyes" if field == "blink" else field
        if t.HasField(field) and not c[device]["enabled"]:
            raise ValueError(f"{device} 已禁用")
    if t.HasField("chassis"):
        ch = t.chassis
        cfg = c["chassis"]
        if ch.mode not in (0, 1, 2, 3):
            raise ValueError("底盘模式无效")
        integer(ch.ttl_ms, 100, 1000, "chassis.ttl_ms")
        number(ch.speed_m_s, -cfg["max_speed_m_s"], cfg["max_speed_m_s"], "speed_m_s")
        number(ch.yaw_rate_rad_s, -cfg["max_yaw_rate_rad_s"], cfg["max_yaw_rate_rad_s"],
               "yaw_rate_rad_s")
        limit = min(cfg["max_steering_rad"], 0.698 if ch.mode == 0 else 1.571)
        number(ch.steering_rad, -limit, limit, "steering_rad")
        if abs(ch.steering_rad) > math.radians(20) and abs(ch.speed_m_s) > 0.525:
            raise ValueError("转向超过 20° 时速度不能超过 0.525 m/s")
        if ch.mode in (0, 1) and ch.yaw_rate_rad_s != 0:
            raise ValueError("阿克曼/斜移模式仅接受线速度和转角")
        if ch.mode in (2, 3) and (ch.speed_m_s != 0 or ch.steering_rad != 0):
            raise ValueError("自旋/驻车模式不能指定线速度或转角")
        if ch.mode == 3 and ch.yaw_rate_rad_s != 0:
            raise ValueError("驻车模式不能运动")
    if t.HasField("neck"):
        n = t.neck
        if not n.ListFields():
            raise ValueError("颈部目标至少指定一个轴")
        for name, field in (("yaw", "yaw_rad"), ("pitch", "pitch_rad")):
            if n.HasField(field):
                axis = c["neck"][name]
                number(getattr(n, field), axis["min_rad"], axis["max_rad"], field)
    for name in ("left_arm", "right_arm"):
        if t.HasField(name):
            arm = getattr(t, name)
            if len(arm.joint_rad) != 4:
                raise ValueError("机械臂目标需要四个关节角度")
            integer(arm.duration_ms, 1, 9999, "duration_ms")
            for i, (angle, joint) in enumerate(zip(arm.joint_rad, c[name]["joints"])):
                number(angle, joint["min_rad"], joint["max_rad"], f"{name}[{i}]")
                integer(arm_pwm(joint, angle), 500, 2500, "PWM")
    if t.HasField("eyes"):
        pose = t.eyes
        for name in ("left", "right"):
            if not pose.HasField(name):
                raise ValueError("双眼姿态必须同时指定左右眼")
            eye = getattr(pose, name)
            number(eye.gaze_x, -1, 1, "gaze_x")
            number(eye.gaze_y, -1, 1, "gaze_y")
            number(eye.openness, 0, f32(1.4), "openness")
            number(eye.brow, -1, 1, "brow")
        number(pose.roll, -f32(0.8), f32(0.8), "roll")
    if t.HasField("blink") and t.blink.HasField("duration"):
        number(t.blink.duration, f32(0.08), 1, "blink.duration")


def validate_action(action, cfg):
    if not re.fullmatch(r"[\w\-]{1,64}", action.name):
        raise ValueError("动作名称只允许文字、数字、下划线和连字符，最长 64 字符")
    integer(action.duration_ms, 1, 600000, "action.duration_ms")
    if not 1 <= len(action.steps) <= 1000:
        raise ValueError("动作需要 1..1000 个时间点")
    last = -1
    for step in action.steps:
        if not last < step.at_ms < action.duration_ms:
            raise ValueError("动作时间点必须递增且早于结束时间")
        validate_targets(step.targets, cfg)
        last = step.at_ms
