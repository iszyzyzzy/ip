<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue'
import { Eye, EyeOff, PlugZap, RefreshCw, Save, Unplug } from 'lucide-vue-next'
import { useInputController } from '@/lib/input-controller'
import { clientConfig, saveClientConfig } from '@/lib/settings'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()
const input = useInputController()
const microphones = ref<MediaDeviceInfo[]>([])
const changingMic = ref(false)
const micError = ref('')

async function refreshMics() {
  try {
    const devices = await navigator.mediaDevices.enumerateDevices()
    microphones.value = devices.filter(d => d.kind === 'audioinput' && d.deviceId && d.deviceId !== 'default')
  } catch (err) { micError.value = String(err) }
}
async function requestMicrophones() {
  micError.value = ''
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true })
    stream.getTracks().forEach(t => t.stop())
    await refreshMics()
  } catch (err) { micError.value = String(err) }
}
async function changeMic(event: Event) {
  const select = event.target as HTMLSelectElement
  changingMic.value = true
  micError.value = ''
  try {
    const id = select.value
    await wan.selectMicrophone(id)
    await saveClientConfig({ microphone_device_id: id })
  } catch (err) {
    select.value = clientConfig.microphone_device_id
    micError.value = String(err)
  } finally { changingMic.value = false }
}
onMounted(() => {
  refreshMics()
  navigator.mediaDevices?.addEventListener('devicechange', refreshMics)
})
onUnmounted(() => navigator.mediaDevices?.removeEventListener('devicechange', refreshMics))
const showToken = ref(false)
const localMsg = ref('')
const robotDraft = ref<Record<string, any> | null>(null)
const savingRobotCfg = ref(false)

watch(() => wan.config.value, cfg => {
  if (cfg) {
    robotDraft.value = JSON.parse(JSON.stringify(cfg))
  }
}, { immediate: true })

async function saveAndConnect() {
  localMsg.value = ''
  await saveClientConfig()
  await wan.connect()
}

async function queryRobots() {
  localMsg.value = ''
  try {
    await saveClientConfig()
    await wan.refreshRobots()
  } catch (err) {
    localMsg.value = String((err as Error).message || err)
  }
}

async function saveRobotConfig() {
  if (!robotDraft.value || savingRobotCfg.value) return
  savingRobotCfg.value = true
  localMsg.value = ''
  try {
    await wan.rpc('update_config', {
      expected_revision: wan.configRevision.value,
      config: robotDraft.value,
    })
    await wan.loadConfig()
    localMsg.value = '车端配置已保存'
  } catch (err) {
    localMsg.value = String((err as Error).message || err)
  } finally {
    savingRobotCfg.value = false
  }
}
</script>

<template>
  <div class="conn-grid dv-scroll">
    <!-- 公网信令与操作员认证 -->
    <section class="dv-card panel-block">
      <div class="dv-section-title">
        <span>公网信令连接</span>
        <span class="dv-pill" :class="wan.online.value ? 'ok' : 'warn'">{{ wan.statusMessage.value }}</span>
      </div>

      <div v-if="localMsg" class="msg-banner">{{ localMsg }}</div>

      <div class="form-grid">
        <label class="field">
          信令服务地址
          <input v-model="clientConfig.signal_url" class="dv-input mono" />
        </label>

        <label class="field">
          车辆 ID
          <input v-model="clientConfig.robot_id" class="dv-input mono" />
        </label>

        <label class="field span-2">
          操作员 Token
          <div class="token-row">
            <input
              v-model="clientConfig.operator_token"
              :type="showToken ? 'text' : 'password'"
              class="dv-input mono"
            />
            <button type="button" class="dv-btn dv-btn-sm" @click="showToken = !showToken">
              <component :is="showToken ? EyeOff : Eye" :size="14" />
            </button>
          </div>
        </label>
      </div>

      <div class="actions-row">
        <button class="dv-btn dv-btn-ok" :disabled="wan.busy.value" @click="saveAndConnect">
          <PlugZap :size="14" /> 保存并连接
        </button>
        <button class="dv-btn" :disabled="wan.busy.value" @click="queryRobots">
          <RefreshCw :size="14" /> 查询在线车辆
        </button>
        <button v-if="wan.online.value" class="dv-btn dv-btn-warn" @click="wan.disconnect()">
          <Unplug :size="14" /> 断开会话
        </button>
      </div>

      <div v-if="wan.robots.value.length" class="robots-list">
        <div
          v-for="r in wan.robots.value"
          :key="r.robot_id"
          class="robot-chip"
          :class="{ selected: clientConfig.robot_id === r.robot_id }"
          @click="clientConfig.robot_id = r.robot_id"
        >
          <strong class="mono">{{ r.robot_id }}</strong>
          <span class="dv-pill" :class="r.online ? (r.busy ? 'warn' : 'ok') : 'danger'">
            {{ r.online ? (r.busy ? '占用中' : '在线空闲') : '离线' }}
          </span>
          <span class="mono cap">
            图:{{ r.capabilities?.video ? 'Y' : 'N' }}
            麦:{{ r.capabilities?.mic ? 'Y' : 'N' }}
            声:{{ r.capabilities?.audio ? 'Y' : 'N' }}
          </span>
        </div>
      </div>
    </section>

    <!-- 桌面端操控与音频设置 -->
    <section class="dv-card panel-block">
      <div class="dv-section-title">
        <span>操控与音频设置</span>
      </div>
      <div class="checks-grid">
        <label class="check-item">
          <input v-model="clientConfig.keyboard_enabled" type="checkbox" @change="saveClientConfig()" />
          启用键盘操控
        </label>
        <label class="check-item">
          <input v-model="clientConfig.gamepad_enabled" type="checkbox" @change="saveClientConfig()" />
          启用本地手柄
          <span v-if="input.activeGamepadName.value" class="dv-pill ok mono">{{ input.activeGamepadName.value }}</span>
        </label>
        <label class="check-item">
          <input v-model="clientConfig.mic_listen_enabled" type="checkbox" @change="saveClientConfig()" />
          收听车端回传音频
        </label>
        <label class="check-item">
          <input v-model="clientConfig.auto_half_duplex" type="checkbox" @change="saveClientConfig()" />
          自动半双工
        </label>
      </div>

      <label class="field">喊话麦克风
        <select class="dv-input" :value="clientConfig.microphone_device_id" :disabled="changingMic || wan.busy.value" @change="changeMic">
          <option value="">系统默认麦克风</option>
          <option v-if="clientConfig.microphone_device_id && !microphones.some(d => d.deviceId === clientConfig.microphone_device_id)" :value="clientConfig.microphone_device_id">已选麦克风（未检测到）</option>
          <option v-for="(mic, i) in microphones" :key="mic.deviceId" :value="mic.deviceId">{{ mic.label || ('麦克风 ' + (i + 1)) }}</option>
        </select>
      </label>
      <button class="dv-btn dv-btn-sm" @click="requestMicrophones">获取麦克风列表</button>
      <div v-if="micError" class="msg-banner">{{ micError }}</div>
      <div class="gear-speed-grid">
        <label class="field">
          低速挡 (m/s)
          <input
            v-model.number="clientConfig.gear_speeds.low"
            type="number"
            min="0.01"
            max="1.5"
            step="0.05"
            class="dv-input mono"
            @change="saveClientConfig()"
          />
        </label>
        <label class="field">
          中速挡 (m/s)
          <input
            v-model.number="clientConfig.gear_speeds.mid"
            type="number"
            min="0.01"
            max="1.5"
            step="0.05"
            class="dv-input mono"
            @change="saveClientConfig()"
          />
        </label>
        <label class="field">
          高速挡 (m/s)
          <input
            v-model.number="clientConfig.gear_speeds.high"
            type="number"
            min="0.01"
            max="1.5"
            step="0.05"
            class="dv-input mono"
            @change="saveClientConfig()"
          />
        </label>
      </div>
    </section>

    <!-- robotd 权威配置 -->
    <section class="dv-card panel-block">
      <div class="dv-section-title">
        <span>车端配置</span>
        <button class="dv-btn dv-btn-sm" :disabled="!wan.online.value" @click="wan.loadConfig()">
          <RefreshCw :size="12" /> 重新读取
        </button>
      </div>

      <div v-if="!robotDraft" class="empty-tip">未连接车辆</div>
      <template v-else>
        <div class="form-grid">
          <label class="field">
            机器人名称
            <input v-model="robotDraft.metadata.name" class="dv-input" />
          </label>
          <label class="field">
            最大线速度 (m/s)
            <input v-model.number="robotDraft.chassis.max_speed_m_s" type="number" step="0.05" min="0.01" max="1.5" class="dv-input mono" />
          </label>
          <label class="field">
            最大自旋角速度 (rad/s)
            <input v-model.number="robotDraft.chassis.max_yaw_rate_rad_s" type="number" step="0.1" min="0.01" max="3.259" class="dv-input mono" />
          </label>
          <label class="field">
            最大转角 (rad)
            <input v-model.number="robotDraft.chassis.max_steering_rad" type="number" step="0.05" min="0.01" max="1.571" class="dv-input mono" />
          </label>
          <label class="field">
            ALSA 扬声器设备
            <input v-model="robotDraft.audio.device" class="dv-input mono" />
          </label>
          <label class="check-item inline-check">
            <input v-model="robotDraft.audio.enabled" type="checkbox" /> 启用车载音频播放
          </label>
        </div>

        <div class="actions-row">
          <button
            class="dv-btn dv-btn-ok"
            :disabled="!wan.online.value || savingRobotCfg"
            @click="saveRobotConfig"
          >
            <Save :size="14" /> 保存车端配置
          </button>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.conn-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding-right: 4px;
}

.panel-block {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.gear-speed-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}

.span-2 {
  grid-column: span 2;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-dim);
}

.token-row {
  display: flex;
  gap: 6px;
}

.actions-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.robots-list {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.robot-chip {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border: 1px solid rgba(0, 216, 255, 0.28);
  border-radius: 4px;
  background: rgba(4, 18, 44, 0.72);
  cursor: pointer;
  font-size: 12px;
}

.robot-chip.selected {
  border-color: var(--cyan-bright);
  background: rgba(12, 54, 114, 0.8);
}

.cap {
  font-size: 11px;
  color: var(--text-dim);
}

.checks-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 8px;
}

.check-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  cursor: pointer;
}

.inline-check {
  align-self: end;
  padding-bottom: 6px;
}

.msg-banner {
  padding: 6px 10px;
  border: 1px solid rgba(0, 216, 255, 0.45);
  background: rgba(8, 42, 92, 0.7);
  border-radius: 3px;
  font-size: 12px;
  color: var(--cyan-bright);
}

.empty-tip {
  color: var(--text-dim);
  font-size: 12px;
}
</style>
