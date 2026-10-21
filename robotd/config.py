"""调试工具读取 Rust 服务端编译时使用的同一份初始配置。"""
import json
from importlib.resources import files


def default_config():
    return json.loads(files("robotd").joinpath("default-config.json").read_text(encoding="utf-8"))
