#!/bin/sh
# 安装已针对运行内核编译的模块，保留首次启动配置供回退。
set -eu
cd "$(dirname "$0")"
test "$(id -u)" = 0 || { echo "请使用 sudo 运行" >&2; exit 1; }
kernel=$(uname -r)
config=/boot/firmware/config.txt
backup=/boot/firmware/config.txt.before-fl7707n
test -e "$backup" || cp -a "$config" "$backup"
install -D -m 644 panel-fl7707n.ko "/lib/modules/$kernel/extra/panel-fl7707n.ko"
depmod -a
install -m 644 fl7707n-dual.dtbo /boot/firmware/overlays/fl7707n-dual.dtbo
if ! grep -qx 'dtoverlay=fl7707n-dual' "$config"; then
    printf '\n[all]\n# 双路 FL7707N 面板\ndtoverlay=fl7707n-dual\n' >> "$config"
fi
printf 'panel_fl7707n\n' > /etc/modules-load.d/fl7707n.conf
install -D -m 644 monitor-power.py /usr/local/libexec/fl7707n-monitor-power.py
install -m 644 show-pattern.py /usr/local/libexec/fl7707n-show-pattern.py
install -m 644 screen-monitor.service screen-pattern.service /etc/systemd/system/
mkdir -p /etc/systemd/journald.conf.d
cat > /etc/systemd/journald.conf.d/fl7707n-monitor.conf <<'JOURNAL'
[Journal]
Storage=persistent
SystemMaxUse=32M
SyncIntervalSec=15s
JOURNAL
mkdir -p /var/log/journal
systemd-tmpfiles --create --prefix /var/log/journal
systemctl restart systemd-journald
journalctl --flush
systemctl daemon-reload
systemctl enable screen-pattern.service screen-monitor.service
# 测试期间用 framebuffer 保留彩条，避免桌面合成器接管这两路输出。
systemctl disable lightdm.service
systemctl --no-block stop lightdm.service
systemctl start screen-monitor.service
echo "开机驱动、彩条和监测已安装；重启后生效。"
