#!/usr/bin/env sh
set -eu

phone=${1:-}
bundle=${EYES_BUNDLE:-"$HOME/eyes/dist/eyes-linux"}
cd "$bundle"
export XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR:-"/run/user/$(id -u)"}
socket="$XDG_RUNTIME_DIR/eyes.sock"
config=$(mktemp)
child=
cleanup() {
    if [ -n "$child" ]; then
        kill -TERM "$child" 2>/dev/null || true
        wait "$child" 2>/dev/null || true
    fi
    rm -f "$config"
}
trap cleanup EXIT
trap 'exit 0' INT TERM

if [ -n "$phone" ]; then
    python3 - "$phone" > "$config" <<'PY'
import ipaddress
import json
import sys

phone = str(ipaddress.IPv4Address(sys.argv[1]))
json.dump({"mode": 2, "phone_ip": phone, "tracking_port": 49983}, sys.stdout)
PY
fi

# 按平台路径选择两路 DSI，避免设备编号随加载顺序变化。
./eyes --headless \
    --drm-device /dev/dri/by-path/platform-1f00118000.dsi-card --left-connector DSI-1 \
    --right-drm-device /dev/dri/by-path/platform-1f00130000.dsi-card --right-connector DSI-2 \
    --assets "$bundle/assets" --settings "$bundle/eyes-settings.json" \
    --socket "$socket" --fps "${EYES_FPS:-60}" &
child=$!

ready=false
for attempt in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 "$child" 2>/dev/null || break
    if [ -S "$socket" ] && ./controller "$socket" state >/dev/null 2>&1; then
        ready=true
        break
    fi
    sleep 1
done
[ "$ready" = true ] || { echo "眼屏进程未就绪，请检查渲染日志" >&2; exit 1; }
if [ -n "$phone" ]; then
    ./controller "$socket" configure "$config"
    ./controller "$socket" save
    ./controller "$socket" connect
else
    ./controller "$socket" idle
    ./controller "$socket" save
fi
wait "$child"
