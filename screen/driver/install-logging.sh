#!/bin/sh
# 日志独立安装，避免为记录重启线索而修改显示驱动或启动参数。
set -eu
cd "$(dirname "$0")"
test "$(id -u)" = 0 || { echo "请使用 sudo 运行" >&2; exit 1; }

install -D -m 644 monitor-power.py /usr/local/libexec/fl7707n-monitor-power.py
install -m 644 screen-monitor.service /etc/systemd/system/screen-monitor.service
mkdir -p /etc/systemd/journald.conf.d /var/log/journal
cat > /etc/systemd/journald.conf.d/90-screen-debug.conf <<'JOURNAL'
[Journal]
Storage=persistent
SystemMaxUse=128M
RuntimeMaxUse=32M
SyncIntervalSec=5s
JOURNAL
systemd-tmpfiles --create --prefix /var/log/journal
systemctl restart systemd-journald
journalctl --flush
systemctl daemon-reload
systemctl enable --now screen-monitor.service
echo "内核日志已启用持久化，供电监测服务已启动并随开机运行。"
echo "上次启动日志：sudo journalctl -k -b -1"
echo "供电采样：sudo journalctl -u screen-monitor.service"
