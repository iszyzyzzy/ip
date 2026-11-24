<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { RotateCcw } from 'lucide-vue-next'
import { useWanClient } from '@/lib/wan-client'

import JoystickPad from '@/components/dashboard/sub/JoystickPad.vue'
import { useInputController } from '@/lib/input-controller'

const wan = useWanClient()

const input = useInputController()
const neckPose = input.neckPose
const eyesFollow = ref(false)

async function zeroNeck() {
  if (!confirm('以当前颈部机械位置作为运行时零点？此操作会先停止运动。')) return
  try {
    await wan.rpc('zero_neck', { mask: 3 })
  } catch (err) {
    wan.errorMessage.value = String((err as Error).message || err)
  }
}

function blink(left: boolean, right: boolean) {
  wan.setTargets({ blink: { left, right } }).catch(() => {})
}

const neckFeedback = computed(() => {
  const n = wan.state.value?.devices?.neck
  return {
    yaw: typeof n?.gm6020?.joint_position_rad === 'number'
      ? `${((n.gm6020.joint_position_rad * 180) / Math.PI).toFixed(1)}°`
      : '—',
    pitch: typeof n?.dm4310?.joint_position_rad === 'number'
      ? `${((n.dm4310.joint_position_rad * 180) / Math.PI).toFixed(1)}°`
      : '—',
  }
})

// ---- 右侧：眼屏参数实时调节 ----
const draft = ref<Record<string, any> | null>(null)
const baseRevision = ref('')
const dirty = ref(false)
const saving = ref(false)
const previewState = ref<'live' | 'pending' | 'error' | 'conflict'>('live')
const previewPhone = ref('')
const gazePad = ref<HTMLDivElement | null>(null)
const gazeDragging = ref(false)
let previewTimer = 0
let previewPending = false
let previewReq: Promise<void> | null = null

const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v))

watch(() => [wan.config.value?.eyes?.settings, wan.configRevision.value] as const, ([settings, rev]) => {
  if (settings && !dirty.value) {
    draft.value = clone(settings)
    if (!draft.value!.phone_ip) draft.value!.phone_ip = '192.168.3.110'
    baseRevision.value = rev
    previewPhone.value = settings.phone_ip ?? ''
  }
}, { immediate: true })

onBeforeUnmount(() => {
  clearTimeout(previewTimer)
})

function cancelPreview() {
  clearTimeout(previewTimer)
  previewTimer = 0
  previewPending = false
}

function changed() {
  dirty.value = true
  if (!wan.online.value || !wan.config.value?.eyes?.enabled || saving.value) return
  previewPending = true
  previewState.value = 'pending'
  if (!previewTimer && !previewReq) {
    previewTimer = window.setTimeout(flushPreview, 50)
  }
}

function flushPreview() {
  clearTimeout(previewTimer)
  previewTimer = 0
  if (!previewPending || !draft.value || !wan.online.value || saving.value) return
  previewPending = false
  if (wan.configRevision.value !== baseRevision.value) {
    previewState.value = 'conflict'
    return
  }
  const settings = clone(draft.value)
  settings.phone_ip = previewPhone.value
  previewReq = wan.rpc('update_eyes', { expected_revision: baseRevision.value, settings })
    .then(() => { previewState.value = 'live' })
    .catch(() => { cancelPreview(); previewState.value = 'error' })
    .finally(() => {
      previewReq = null
      if (previewPending) changed()
    })
}

function updateGaze(e: PointerEvent) {
  if (!gazePad.value || !draft.value) return
  const rect = gazePad.value.getBoundingClientRect()
  const x = Math.max(-1, Math.min(1, ((e.clientX - rect.left) / rect.width) * 2 - 1))
  const y = Math.max(-1, Math.min(1, ((e.clientY - rect.top) / rect.height) * 2 - 1))
  draft.value.simulator.target_x = Math.round(x * 100) / 100
  draft.value.simulator.target_y = Math.round(y * 100) / 100
  draft.value.simulator.eye_motion = false
  changed()
}

function onGazeMove(e: PointerEvent) {
  if (gazeDragging.value) updateGaze(e)
}

function colorHex(obj: any) {
  return '#' + ['r', 'g', 'b']
    .map(k => Math.round((obj?.[k] ?? 1) * 255).toString(16).padStart(2, '0'))
    .join('')
}

function setColor(key: string, hex: string) {
  if (!draft.value) return
  draft.value[key] = Object.fromEntries(
    ['r', 'g', 'b'].map((k, i) => [k, parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16) / 255]),
  )
  changed()
}

function resetEyes() {
  if (wan.config.value?.eyes?.settings) {
    draft.value = clone(wan.config.value.eyes.settings)
    previewPhone.value = wan.config.value.eyes.settings.phone_ip ?? ''
  }
  baseRevision.value = wan.configRevision.value
  dirty.value = false
  changed()
}

async function applyEyes(connectFace = false) {
  if (!draft.value || saving.value) return
  if (wan.configRevision.value !== baseRevision.value) {
    wan.errorMessage.value = '配置已在别处变更，请先点击「重新载入」'
    return
  }
  const settings = clone(draft.value)
  const ip = String(settings.phone_ip ?? '').trim()
  const validIp = /^\d{1,3}(\.\d{1,3}){3}$/.test(ip) && ip.split('.').every(v => Number(v) <= 255)
  if ((connectFace || settings.mode === 'IFACIALMOCAP') && !validIp) {
    wan.errorMessage.value = '请输入有效的面捕手机 IPv4 地址'
    return
  }
  settings.phone_ip = settings.mode === 'IFACIALMOCAP' || connectFace ? ip : previewPhone.value
  if (connectFace) settings.mode = 'IFACIALMOCAP'
  saving.value = true
  cancelPreview()
  try {
    await previewReq
    const res = await wan.rpc<{ revision: string }>('update_eyes', {
      expected_revision: baseRevision.value,
      settings,
      save: !connectFace,
      connect_tracking: connectFace,
    })
    draft.value = settings
    previewPhone.value = settings.phone_ip
    previewState.value = 'live'
    if (!connectFace) {
      baseRevision.value = res.revision
      dirty.value = false
      await wan.loadConfig()
    }
  } catch (err) {
    previewState.value = 'error'
    wan.errorMessage.value = String((err as Error).message || err)
  } finally {
    saving.value = false
  }
}

const eyeState = computed(() => wan.state.value?.devices?.eyes)
const eyeSliders = [
  { key: 'scale', label: '眼睛大小', min: 0.5, max: 1.5, step: 0.01 },
  { key: 'iris_size', label: '虹膜大小', min: 0.1, max: 0.22, step: 0.001 },
  { key: 'pupil', label: '瞳孔大小', min: 0.2, max: 0.65, step: 0.01 },
  { key: 'expression', label: '神态', min: -1, max: 1, step: 0.05 },
]
</script>

<template>
  <div class="head-eyes-grid dv-scroll">
    <!-- 左栏：颈部控制 -->
    <section class="dv-card panel-box">
      <div class="dv-section-title">
        <span>头部云台与零位</span>
        <span class="dv-pill mono" :class="wan.config.value?.neck?.enabled ? 'ok' : 'warn'">
          {{ wan.config.value?.neck?.enabled ? '已启用' : '未启用' }}
        </span>
      </div>

      <JoystickPad class="neck-pad" :size="190" :spring="false"
        :disabled="input.headDisabled.value" :disabled-label="input.headStatus.value"
        :model-x="input.neckStick.x" :model-y="input.neckStick.y" top-label="抬头"
        @change="(x, y) => input.setNeckStick(x, y, eyesFollow)" />

      <div class="readout-grid mono">
        <div class="readout-item">
          <span>偏航目标 / 实际</span>
          <strong>{{ ((neckPose.yaw * 180) / Math.PI).toFixed(0) }}° / {{ neckFeedback.yaw }}</strong>
        </div>
        <div class="readout-item">
          <span>俯仰目标 / 实际</span>
          <strong>{{ ((neckPose.pitch * 180) / Math.PI).toFixed(0) }}° / {{ neckFeedback.pitch }}</strong>
        </div>
      </div>

      <label class="switch-row">
        <input v-model="eyesFollow" type="checkbox" /> 眼屏注视跟随头部摇杆
      </label>

      <div class="btn-grid-3">
        <button class="dv-btn dv-btn-sm" :disabled="!wan.controlling.value" @click="blink(true, true)">眨眼</button>
        <button class="dv-btn dv-btn-sm" :disabled="!wan.controlling.value" @click="blink(false, true)">眨右眼</button>
        <button class="dv-btn dv-btn-sm" :disabled="!wan.controlling.value" @click="blink(true, false)">眨左眼</button>
      </div>

      <div class="btn-grid-2">
        <button class="dv-btn" :disabled="!input.canCenterHead.value" @click="input.centerHead(eyesFollow)">
          <RotateCcw :size="14" /> 头部回中
        </button>
        <button class="dv-btn dv-btn-warn" :disabled="!wan.controlling.value || !wan.config.value?.neck?.enabled" @click="zeroNeck">
          设为运行时零点
        </button>
      </div>
    </section>

    <!-- 右栏：眼屏参数与面捕 -->
    <section class="dv-card panel-box">
      <div class="dv-section-title">
        <span>眼屏渲染参数</span>
        <span class="dv-pill mono">
          {{ previewState === 'pending' ? '同步中' : previewState === 'error' ? '失败' : previewState === 'conflict' ? '版本冲突' : '实时已同步' }}
        </span>
      </div>

      <div v-if="!draft" class="empty-tip">未连接车辆</div>
      <template v-else>
        <div class="btn-grid-3">
          <button
            v-for="m in [
              { v: 'IDLE', l: '自主模式' },
              { v: 'IFACIALMOCAP', l: '手机面捕' },
              { v: 'SIMULATOR', l: '定睛注视' },
            ]"
            :key="m.v"
            class="dv-btn dv-btn-sm"
            :class="{ active: draft.mode === m.v }"
            @click="draft.mode = m.v; changed()"
          >
            {{ m.l }}
          </button>
        </div>

        <div v-if="draft.mode === 'SIMULATOR'" class="gaze-section">
          <div class="gaze-head">
            <span>定睛目标</span>
            <button class="dv-btn dv-btn-sm" @click="draft.simulator.target_x = 0; draft.simulator.target_y = 0; changed()">回中</button>
          </div>
          <div
            ref="gazePad"
            class="gaze-rect"
            @pointerdown.prevent="gazeDragging = true; ($event.target as Element)?.setPointerCapture?.($event.pointerId); updateGaze($event)"
            @pointermove="onGazeMove"
            @pointerup="gazeDragging = false"
            @pointercancel="gazeDragging = false"
          >
            <div
              class="pad-knob"
              :style="{ left: `${50 + (draft.simulator.target_x ?? 0) * 45}%`, top: `${50 + (draft.simulator.target_y ?? 0) * 45}%` }"
            />
          </div>
          <label class="switch-row">
            <input v-model="draft.simulator.eye_motion" type="checkbox" @change="changed()" /> 目标附近轻微眼跳
          </label>
        </div>

        <div v-if="draft.mode === 'IFACIALMOCAP'" class="field-group">
          <label>面捕手机 IPv4 (UDP {{ draft.tracking_port }})</label>
          <div class="ip-row">
            <input v-model="draft.phone_ip" class="dv-input mono" placeholder="192.168.3.110" @input="dirty = true" />
            <button class="dv-btn dv-btn-sm dv-btn-ok" :disabled="!wan.online.value || saving || !wan.config.value?.eyes?.enabled" @click="applyEyes(true)">
              连接面捕
            </button>
          </div>
          <span v-if="eyeState" class="sub-info mono">
            {{ eyeState.trackingStatus ?? eyeState.tracking_status ?? '未连接' }} · 已收 {{ eyeState.trackingPackets ?? eyeState.tracking_packets ?? 0 }} 包
          </span>
        </div>

        <div class="sliders-grid">
          <label v-for="s in eyeSliders" :key="s.key" class="slider-item">
            <div class="slider-head">
              <span>{{ s.label }}</span>
              <span class="mono">{{ Number(draft[s.key]).toFixed(2) }}</span>
            </div>
            <input v-model.number="draft[s.key]" type="range" :min="s.min" :max="s.max" :step="s.step" @input="changed()" />
          </label>
        </div>

        <div class="btn-grid-2">
          <label class="color-item">
            虹膜颜色
            <input type="color" :value="colorHex(draft.iris)" @input="setColor('iris', ($event.target as HTMLInputElement).value)" />
          </label>
          <label class="color-item">
            背景颜色
            <input type="color" :value="colorHex(draft.background)" @input="setColor('background', ($event.target as HTMLInputElement).value)" />
          </label>
        </div>

        <div class="btn-grid-2">
          <label class="switch-row">
            <input v-model="draft.auto_blink" type="checkbox" @change="changed()" /> 自动眨眼
          </label>
          <label class="switch-row">
            <input v-model="draft.jelly.enabled" type="checkbox" @change="changed()" /> 弹性晃动
          </label>
        </div>

        <div v-if="draft.auto_blink || draft.jelly.enabled" class="sliders-grid">
          <label v-if="draft.auto_blink" class="slider-item">
            <div class="slider-head">
              <span>眨眼间隔</span>
              <span class="mono">{{ Number(draft.blink_interval).toFixed(1) }} s</span>
            </div>
            <input v-model.number="draft.blink_interval" type="range" min="0.5" max="20" step="0.1" @input="changed()" />
          </label>
          <label v-if="draft.auto_blink" class="slider-item">
            <div class="slider-head">
              <span>眨眼速度</span>
              <span class="mono">{{ Number(draft.blink_duration).toFixed(2) }} s</span>
            </div>
            <input v-model.number="draft.blink_duration" type="range" min="0.08" max="1" step="0.01" @input="changed()" />
          </label>
          <label v-if="draft.jelly.enabled" class="slider-item">
            <div class="slider-head">
              <span>弹性强度</span>
              <span class="mono">{{ Number(draft.jelly.strength).toFixed(2) }}</span>
            </div>
            <input v-model.number="draft.jelly.strength" type="range" min="0" max="1" step="0.05" @input="changed()" />
          </label>
        </div>

        <button
          class="dv-btn dv-btn-sm"
          :disabled="!wan.controlling.value || !wan.config.value?.eyes?.enabled"
          @click="blink(true, true)"
        >
          试一下眨眼
        </button>

        <div class="btn-grid-2">
          <button class="dv-btn" :disabled="saving" @click="resetEyes">重新载入</button>
          <button class="dv-btn dv-btn-ok" :disabled="!wan.online.value || saving" @click="applyEyes(false)">
            {{ saving ? '保存中…' : '保存参数到车端' }}
          </button>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.head-eyes-grid {
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: 12px;
  height: 100%;
  padding-right: 4px;
}

.panel-box {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.neck-pad { align-self: center; }

.pad-knob {
  position: absolute;
  width: 22px;
  height: 22px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  background: var(--cyan-bright);
  box-shadow: 0 0 12px var(--cyan-bright);
  pointer-events: none;
}

.gaze-rect {
  position: relative;
  height: 120px;
  border: 1px solid rgba(0, 216, 255, 0.4);
  border-radius: 4px;
  background: rgba(3, 12, 30, 0.9);
  touch-action: none;
  cursor: pointer;
}

.gaze-section {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.gaze-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 12px;
  color: var(--text-dim);
}

.readout-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
}

.readout-item {
  padding: 6px 8px;
  border: 1px solid rgba(0, 216, 255, 0.2);
  border-radius: 3px;
  background: rgba(4, 16, 40, 0.72);
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 11px;
}

.readout-item span { color: var(--text-dim); }
.readout-item strong { color: var(--cyan-bright); font-size: 13px; }

.switch-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  cursor: pointer;
}

.btn-grid-3 {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 6px;
}

.btn-grid-2 {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-dim);
}

.ip-row {
  display: flex;
  gap: 6px;
}

.sub-info {
  font-size: 11px;
  color: var(--cyan-bright);
}

.sliders-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px 14px;
}

.slider-item {
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 12px;
}

.slider-head {
  display: flex;
  justify-content: space-between;
  color: var(--text-dim);
}

.slider-item input[type='range'] {
  accent-color: var(--cyan);
}

.color-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 10px;
  border: 1px solid rgba(0, 216, 255, 0.24);
  border-radius: 3px;
  background: rgba(4, 16, 40, 0.7);
  font-size: 12px;
}

.empty-tip {
  color: var(--text-dim);
  font-size: 12px;
}
</style>
