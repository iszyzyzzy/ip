"""GPU image regression: every individual layer remains visible by itself.
Run after cargo build: python scripts/check_layers.py
Only writes disposable files inside target/layer-check.
"""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parents[1]
out = root / "target" / "layer-check"
out.mkdir(parents=True, exist_ok=True)
names = ["iris", "pupil", "highlights", "sclera", "upper_lid", "lower_lid", "brow"]
baseline = None
for selected in [None] + names:
    label = selected or "none"
    settings = {
        "mode": "Simulator", "show_controls": False, "auto_blink": False,
        "simulator": {"eye_motion": False, "show_target": False},
        "layers": {name: name == selected for name in names},
        "glow": 0.8 if selected is None else 0,
    }
    config = out / f"{label}.json"
    shot = out / f"{label}.ppm"
    config.write_text(json.dumps(settings), encoding="utf-8")
    subprocess.run([str(root / "target/debug/eyes.exe"), "--no-grpc", "--assets", str(root / "assets"), "--settings", str(config), "--screenshot", str(shot), "--exit-after", "1.3"], check=True, cwd=root, timeout=20, creationflags=subprocess.CREATE_NO_WINDOW)
    header, dims, depth, pixels = shot.read_bytes().split(b"\n", 3)
    assert header == b"P6" and depth == b"255"
    if selected is None:
        baseline = pixels
        assert pixels == pixels[:3] * (len(pixels) // 3), "all hidden must show only background"
    else:
        changed = sum(a != b for a, b in zip(pixels, baseline))
        assert changed > 100, f"{selected} is invisible when rendered independently"
        print(f"PASS {selected}: {changed} changed RGB bytes", flush=True)
print("PASS: all seven layers independently visible; hide-all leaves no remnants")
