"""从唯一的眼屏协议源生成接口，避免手工维护两份协议。"""
from pathlib import Path
import sys
import shutil
import grpc_tools
from grpc_tools import protoc

root = Path(__file__).resolve().parent.parent
out = root / "robotd/pb"
rc = protoc.main([
    "protoc", f"-I{root / 'robotd/proto'}", f"-I{root / 'eyes/proto'}",
    f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
    f"--python_out={out}", f"--grpc_python_out={out}",
    str(root / "robotd/proto/robot.proto"), str(root / "eyes/proto/eyes.proto"),
])
if rc:
    sys.exit(rc)
shutil.copyfile(root / "eyes/proto/eyes.proto", out / "eyes.proto")
for path in out.glob("*_pb2*.py"):
    s = path.read_text(encoding="utf-8")
    for name in ("eyes_pb2", "robot_pb2"):
        s = s.replace(f"import {name} as", f"from . import {name} as")
    path.write_text(s, encoding="utf-8")
