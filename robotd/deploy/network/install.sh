#!/bin/sh
set -eu
cd "$(dirname "$0")"
test "$(id -u)" = 0
command -v netplan >/dev/null
command -v nmcli >/dev/null
command -v dnsmasq >/dev/null
python3 -c 'import yaml'

install -d -m 700 /var/lib/robot-network /run/robot-network
install -m 755 robot-network.py /usr/local/sbin/robot-network

# 安装时取当前连接作恢复基线，热点密码和 UUID 保留在设备上。
python3 - <<'PY'
import json
from pathlib import Path
import secrets
import string
import subprocess
import uuid

config = Path('/etc/robot-network.json')
if config.exists():
    raise SystemExit(0)
client = subprocess.check_output(
    ['nmcli', '-g', 'GENERAL.CON-UUID', 'device', 'show', 'wlan0'], text=True).strip()
uuid.UUID(client)
hotspot = str(uuid.uuid4())
password = ''.join(secrets.choice(string.ascii_letters + string.digits) for _ in range(12))
config.touch(mode=0o600)
config.write_text(json.dumps({'mode': 'fallback', 'client_uuid': client,
                              'hotspot_uuid': hotspot}, indent=2) + '\n')
profile = Path('/var/lib/robot-network/hotspot.nmconnection')
profile.touch(mode=0o600)
profile.write_text(f'''[connection]
id=robot-control
uuid={hotspot}
type=wifi
interface-name=wlan0
autoconnect=false

[wifi]
mode=ap
ssid=ip-rpi-control
band=bg
channel=6

[wifi-security]
key-mgmt=wpa-psk
psk={password}

[ipv4]
method=shared
address1=192.168.50.1/24
never-default=true

[ipv6]
method=disabled
''')
PY

if [ ! -d /var/lib/robot-network/netplan ]; then
    robot-network backup
fi
robot-network prepare
install -m 644 robot-network-prepare.service robot-network.service robot-network.timer /etc/systemd/system/
systemctl daemon-reload
systemctl enable robot-network-prepare.service
systemctl enable --now robot-network.timer
systemctl start robot-network.service
sync
printf '%s\n' '网络恢复已安装，热点 ip-rpi-control / 192.168.50.1'
