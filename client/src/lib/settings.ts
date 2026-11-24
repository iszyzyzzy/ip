import { invoke } from '@tauri-apps/api/core'
import { reactive } from 'vue'
import { isTauriRuntime } from './signal'
import type { ClientConfig } from './types'

const STORAGE_KEY = 'robot-teleop-client-config-v1'

export const DEFAULT_CONFIG: ClientConfig = {
  signal_url: 'ws://127.0.0.1:8765/v1/signal',
  operator_token: '',
  robot_id: 'car-01',
  keyboard_enabled: true,
  gamepad_enabled: true,
  speed_gear: 'low',
  gear_speeds: { low: 0.15, mid: 0.3, high: 0.6 },
  microphone_device_id: '',
  mic_listen_enabled: true,
  auto_half_duplex: true,
}

export const clientConfig = reactive<ClientConfig>({
  ...DEFAULT_CONFIG,
  gear_speeds: { ...DEFAULT_CONFIG.gear_speeds },
})

let loaded = false

export async function loadClientConfig(): Promise<ClientConfig> {
  if (loaded) return clientConfig
  loaded = true
  try {
    let raw: Partial<ClientConfig> | null = null
    if (isTauriRuntime()) {
      raw = await invoke<Partial<ClientConfig> | null>('load_client_config')
    } else {
      const text = localStorage.getItem(STORAGE_KEY)
      raw = text ? JSON.parse(text) : null
    }
    if (raw && typeof raw === 'object') {
      Object.assign(clientConfig, DEFAULT_CONFIG, raw)
      clientConfig.gear_speeds = {
        ...DEFAULT_CONFIG.gear_speeds,
        ...(raw.gear_speeds ?? {}),
      }
    }
  } catch {
    // 配置读取失败时沿用默认值，不阻塞客户端启动。
  }
  return clientConfig
}

export async function saveClientConfig(patch?: Partial<ClientConfig>): Promise<void> {
  if (patch) {
    Object.assign(clientConfig, patch)
    if (patch.gear_speeds) {
      clientConfig.gear_speeds = { ...clientConfig.gear_speeds, ...patch.gear_speeds }
    }
  }
  const snapshot: ClientConfig = {
    ...clientConfig,
    gear_speeds: { ...clientConfig.gear_speeds },
  }
  try {
    if (isTauriRuntime()) {
      await invoke('save_client_config', { config: snapshot })
    }
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot))
  } catch {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot))
  }
}
