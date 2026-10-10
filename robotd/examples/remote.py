"""上层遥控接入示例：更新底盘目标并明确使用指令 TTL。"""
import time
import grpc
from robotd.client import Session
from robotd.pb import robot_pb2 as pb, robot_pb2_grpc as pg

with grpc.insecure_channel("127.0.0.1:50051", options=[("grpc.enable_http_proxy", 0)]) as channel:
    with Session(pg.RobotControlStub(channel), owner="remote") as session:
        # 在此读取实际手柄；例子只持续 2 秒，退出时释放控制权并停止。
        end = time.monotonic() + 2
        while time.monotonic() < end:
            session.send(pb.Targets(chassis=pb.ChassisTarget(
                mode=pb.ACKERMANN, speed_m_s=0.1, ttl_ms=200)))
            time.sleep(0.02)
