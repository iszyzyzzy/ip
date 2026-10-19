"""把厂家 DSI_CMD / DSI_PA 表转换为驱动常量，不改写寄存器参数。"""
import re
from pathlib import Path

root = Path(__file__).resolve().parent
source = next(root.parent.glob("*.TXT"))
commands = []
for line in source.read_text(encoding="gb18030", errors="replace").splitlines():
    line = line.split("//", 1)[0]
    match = re.search(r"DSI_(CMD|PA)\(\s*(0[xX][0-9a-fA-F]+)\s*\)", line)
    if match:
        kind, value = match.groups()
        if kind == "CMD":
            commands.append({"data": [int(value, 16)], "delay": 0})
        else:
            commands[-1]["data"].append(int(value, 16))
    delay = re.search(r"DelayX1ms\((\d+)\)", line)
    if delay:
        commands[-1]["delay"] = int(delay.group(1))

assert len(commands) == 23
assert max(len(c["data"]) for c in commands) <= 64
assert commands[-2] == {"data": [0x11], "delay": 250}
assert commands[-1] == {"data": [0x29], "delay": 50}
lines = ["/* 自动生成，来源为厂家 2lane V5.5 初始化表。 */",
         "static const struct fl7707n_cmd fl7707n_init[] = {"]
for command in commands:
    data = ", ".join(f"0x{v:02x}" for v in command["data"])
    lines.append(f'\t{{ {len(command["data"])}, {{ {data} }}, {command["delay"]} }},')
lines.append("};\n")
root.joinpath("fl7707n-init.h").write_text("\n".join(lines), encoding="utf-8")
print(f"已生成 {len(commands)} 条初始化命令")
