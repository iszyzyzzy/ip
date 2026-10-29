#!/usr/bin/env sh
# 通过树莓派上的 CMSIS-DAP 烧录；先保留整片Flash备份，再校验写入。
set -eu
firmware=${1:?用法：sh flash-pi.sh /绝对路径/firmware.hex [备份目录]}
firmware=$(realpath "$firmware")
[ -r "$firmware" ] || { echo "固件不可读：$firmware" >&2; exit 1; }
backup_dir=${2:-"$HOME/h7-backups"}
mkdir -p "$backup_dir"
backup_dir=$(realpath "$backup_dir")
backup="$backup_dir/h723-$(date +%Y%m%d-%H%M%S)-$$.bin"
command -v openocd >/dev/null || { echo "请先安装 openocd：sudo apt install openocd" >&2; exit 1; }
echo "CMSIS-DAP / SWD 1000kHz；H723 Flash 0x08000000，1MiB"
echo "固件：$firmware"
sha256sum "$firmware"
echo "原固件备份：$backup"
# 花括号路径传给Tcl前拒绝控制字符，避免路径被误当作调试命令。
case "$firmware$backup" in *'{'*|*'}'*|*'\'*|*'
'*) echo "路径不能含花括号、反斜杠或换行" >&2; exit 1;; esac
openocd -f interface/cmsis-dap.cfg -c 'transport select swd' \
    -f target/stm32h7x.cfg -c 'adapter speed 1000' \
    -c 'init' -c 'reset halt' -c 'flash info 0' \
    -c "dump_image {$backup} 0x08000000 0x100000" \
    -c "program {$firmware} verify reset exit"
echo "烧录及校验完成；原Flash已备份到 $backup"
