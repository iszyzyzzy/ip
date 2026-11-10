"""检查配置恢复和热点切换条件，所有网络动作均隔离。"""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('robot_network',
                                             Path(__file__).with_name('robot-network.py'))
network = importlib.util.module_from_spec(spec)
spec.loader.exec_module(network)

VALID = b'''network:
  version: 2
  renderer: NetworkManager
  wifis:
    wlan0:
      dhcp4: true
      access-points:
        fixture-network:
          password: test-password
'''


class NetworkRecoveryTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(dir='/run')
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        for name in ['NETPLAN', 'STATE', 'RUN']:
            path = root / name
            path.mkdir()
            setting = patch.object(network, name, path)
            setting.start()
            self.addCleanup(setting.stop)
        for name in ['CONFIG', 'PROFILE']:
            setting = patch.object(network, name, root / name)
            setting.start()
            self.addCleanup(setting.stop)
        self.config = {'mode': 'fallback', 'client_uuid': 'client', 'hotspot_uuid': 'hotspot'}
        network.CONFIG.write_text(json.dumps(self.config))

    def test_configuration_errors(self):
        for data, expected in [(VALID, True), (b'', False),
                               (b'Section "OutputClass"\n', False),
                               (VALID + b'  unknown-property: true\n', False)]:
            with self.subTest(data=data):
                (network.NETPLAN / 'network.yaml').write_bytes(data)
                self.assertEqual(network.configuration_valid(network.NETPLAN), expected)

    def test_restore_preserves_damaged_files(self):
        baseline = network.STATE / 'netplan'
        baseline.mkdir()
        (baseline / 'network.yaml').write_bytes(VALID)
        (network.NETPLAN / '01-recovery.yaml').write_text('Section "OutputClass"\n')
        (network.NETPLAN / '90-NM.yaml').write_bytes(b'')
        actual_command = network.command

        def isolated_command(*args, **kwargs):
            if args == ('netplan', 'generate'):
                return subprocess.CompletedProcess(args, 0)
            return actual_command(*args, **kwargs)

        with patch.object(network, 'command', side_effect=isolated_command):
            self.assertTrue(network.restore())
        self.assertEqual((network.NETPLAN / 'network.yaml').read_bytes(), VALID)
        self.assertFalse((network.NETPLAN / '01-recovery.yaml').exists())
        archive = list(network.STATE.glob('damaged-*'))
        self.assertEqual(len(archive), 1)
        self.assertIn('OutputClass', (archive[0] / '01-recovery.yaml').read_text())

    def test_fallback_waits_then_starts_hotspot(self):
        with patch.object(network, 'install_hotspot'), \
             patch.object(network, 'restore', return_value=False), \
             patch.object(network, 'active_uuid', return_value=''), \
             patch.object(network, 'has_address', return_value=False), \
             patch.object(network, 'command') as command, \
             patch.object(network.time, 'monotonic', return_value=100) as clock:
            network.tick()
            self.assertFalse(any('up' in call.args for call in command.call_args_list))
            clock.return_value = 189
            network.tick()
            self.assertFalse(any('up' in call.args for call in command.call_args_list))
            clock.return_value = 190
            network.tick()
            command.assert_any_call('nmcli', '--wait', '25', 'connection', 'up',
                                    'uuid', 'hotspot', 'ifname', 'wlan0')

    def test_online_connection_and_active_hotspot_are_preserved(self):
        for active, online in [('client', True), ('hotspot', False)]:
            with self.subTest(active=active), \
                 patch.object(network, 'install_hotspot'), \
                 patch.object(network, 'restore', return_value=False), \
                 patch.object(network, 'active_uuid', return_value=active), \
                 patch.object(network, 'has_address', return_value=online), \
                 patch.object(network, 'command') as command:
                network.tick()
                self.assertFalse(any('up' in call.args for call in command.call_args_list))

    def test_hotspot_mode_switches_even_when_client_is_online(self):
        self.config['mode'] = 'hotspot'
        network.CONFIG.write_text(json.dumps(self.config))
        with patch.object(network, 'install_hotspot'), \
             patch.object(network, 'restore', return_value=False), \
             patch.object(network, 'active_uuid', return_value='client'), \
             patch.object(network, 'has_address', return_value=True), \
             patch.object(network, 'command') as command:
            network.tick()
            command.assert_any_call('nmcli', '--wait', '25', 'connection', 'up',
                                    'uuid', 'hotspot', 'ifname', 'wlan0')


if __name__ == '__main__':
    unittest.main()
