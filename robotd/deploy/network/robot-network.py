#!/usr/bin/python3
"""树莓派网络配置恢复与遥控热点。"""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time

import yaml

STATE = Path('/var/lib/robot-network')
RUN = Path('/run/robot-network')
NETPLAN = Path('/etc/netplan')
CONFIG = Path('/etc/robot-network.json')
PROFILE = Path('/run/NetworkManager/system-connections/robot-control.nmconnection')


def log(message):
    print(message, flush=True)


def command(*args, check=True):
    return subprocess.run(args, text=True, capture_output=True, check=check, timeout=40)


def atomic_write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + '.tmp')
    with temporary.open('wb') as output:
        os.chmod(temporary, 0o600)
        output.write(data)
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(path)
    descriptor = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def configuration_valid(directory):
    """空 YAML 也能通过 Netplan，因此额外确认客户端 Wi-Fi 配置存在。"""
    has_wifi = False
    try:
        for path in directory.glob('*.yaml'):
            document = yaml.safe_load(path.read_text())
            if document is None:
                continue
            network = document['network']
            has_wifi |= bool(network.get('wifis', {}).get('wlan0', {}).get('access-points'))
        if not has_wifi:
            return False
        # 只在内存文件系统生成，避免周期检查反复写卡或改动运行中的连接。
        with tempfile.TemporaryDirectory(prefix='netplan-', dir=RUN) as temporary:
            root = Path(temporary)
            shutil.copytree(directory, root / 'etc/netplan')
            result = command('netplan', 'generate', '--root-dir', temporary, check=False)
            return result.returncode == 0
    except (yaml.YAMLError, KeyError, TypeError, AttributeError, UnicodeError):
        return False


def backup():
    if not configuration_valid(NETPLAN):
        raise RuntimeError('当前 Netplan 无效，未更新恢复基线')
    config = json.loads(CONFIG.read_text())
    if active_uuid('wlan0') != config['client_uuid'] or not has_address('wlan0'):
        raise RuntimeError('客户端 Wi-Fi 尚未连通，未更新恢复基线')
    staging = STATE / 'netplan.new'
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir(mode=0o700)
    for path in NETPLAN.glob('*.yaml'):
        atomic_write(staging / path.name, path.read_bytes())
    previous = STATE / 'netplan.previous'
    if previous.exists():
        shutil.rmtree(previous)
    baseline = STATE / 'netplan'
    if baseline.exists():
        baseline.rename(previous)
    staging.rename(baseline)
    os.sync()
    log('已备份当前连通的 Netplan 配置')


def restore():
    if configuration_valid(NETPLAN):
        return False
    baseline = STATE / 'netplan'
    if not configuration_valid(baseline):
        raise RuntimeError('恢复基线无效，停止覆写；仍可使用独立热点配置')
    archive = STATE / ('damaged-' + time.strftime('%Y%m%d-%H%M%S'))
    archive.mkdir(mode=0o700)
    for path in NETPLAN.glob('*.yaml'):
        path.rename(archive / path.name)
    for path in baseline.glob('*.yaml'):
        atomic_write(NETPLAN / path.name, path.read_bytes())
    command('netplan', 'generate')
    log(f'已恢复 Netplan，异常配置保存在 {archive}')
    return True


def active_uuid(device):
    return command('nmcli', '-g', 'GENERAL.CON-UUID', 'device', 'show', device,
                   check=False).stdout.strip()


def has_address(device):
    state = command('nmcli', '-g', 'GENERAL.STATE', 'device', 'show', device,
                    check=False).stdout
    if not state.startswith('100'):
        return False
    result = command('ip', '-j', 'address', 'show', 'dev', device, check=False)
    if result.returncode:
        return False
    return any(address['scope'] == 'global'
               for link in json.loads(result.stdout) for address in link['addr_info'])


def install_hotspot():
    data = (STATE / 'hotspot.nmconnection').read_bytes()
    if not PROFILE.exists() or PROFILE.read_bytes() != data:
        atomic_write(PROFILE, data)


def prepare():
    install_hotspot()
    try:
        restore()
    except RuntimeError as error:
        # 恢复基线也损坏时，热点仍须能独立启动。
        log(str(error))


def tick():
    config = json.loads(CONFIG.read_text())
    install_hotspot()
    repaired = False
    try:
        repaired = restore()
    except RuntimeError as error:
        log(str(error))
    command('nmcli', 'connection', 'load', str(PROFILE))
    hotspot_active = active_uuid('wlan0') == config['hotspot_uuid']
    offline = RUN / 'offline-since'
    if config['mode'] == 'fallback':
        if hotspot_active:
            return
        if repaired:
            command('nmcli', 'connection', 'reload')
            if not has_address('wlan0') and not has_address('eth0'):
                command('nmcli', '--wait', '20', 'connection', 'up',
                        'uuid', config['client_uuid'], check=False)
        if has_address('wlan0') or has_address('eth0'):
            offline.unlink(missing_ok=True)
            return
        now = time.monotonic()
        if not offline.exists():
            offline.write_text(str(now))
            log('局域网连接丢失，90 秒后启动救援热点')
            return
        if now - float(offline.read_text()) < 90:
            return
    if not hotspot_active:
        command('nmcli', '--wait', '25', 'connection', 'up',
                'uuid', config['hotspot_uuid'], 'ifname', 'wlan0')
        log('遥控热点已开启，地址 192.168.50.1')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'tick', 'backup', 'mode'])
    parser.add_argument('mode', nargs='?', choices=['fallback', 'hotspot'])
    args = parser.parse_args()
    RUN.mkdir(mode=0o700, parents=True, exist_ok=True)
    if args.action == 'mode':
        if args.mode is None:
            parser.error('mode 需要指定 fallback 或 hotspot')
        config = json.loads(CONFIG.read_text())
        config['mode'] = args.mode
        atomic_write(CONFIG, (json.dumps(config, indent=2) + '\n').encode())
        log(f'模式已设为 {args.mode}，下次周期检查生效')
    else:
        {'prepare': prepare, 'tick': tick, 'backup': backup}[args.action]()


if __name__ == '__main__':
    main()
