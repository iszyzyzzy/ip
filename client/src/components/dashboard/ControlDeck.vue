<script setup lang="ts">
import VoiceDialog from "./VoiceDialog.vue"
import SpeakerPanel from "./SpeakerPanel.vue"
import { computed } from 'vue'
import {
  Hand,
  Mic,
  MicOff,
  OctagonX,
  RotateCcw,
  Settings,
  Unlock,
  Volume2,
  VolumeX,
} from 'lucide-vue-next'
import JoystickPad from '@/components/dashboard/sub/JoystickPad.vue'
import { useInputController } from '@/lib/input-controller'
import { clientConfig, saveClientConfig } from '@/lib/settings'
import type { ChassisMode, SpeedGear } from '@/lib/types'
import { useWanClient } from '@/lib/wan-client'

const emit = defineEmits<{ openConfig: [] }>()
const wan = useWanClient()
const input = useInputController()
const modes: { value: ChassisMode, label: string }[] = [
  { value: 'PARK', label: '驻车' },
  { value: 'ACKERMANN', label: '阿克曼' },
  { value: 'CRAB', label: '斜移' },
  { value: 'SPIN', label: '自旋' },
]

const gears: { value: SpeedGear, label: string }[] = [
  { value: 'low', label: '低速' },
  { value: 'mid', label: '中速' },
  { value: 'high', label: '高速' },
]

const latched = computed(() => !!wan.state.value?.latched)
const parkLocked = computed(() => !wan.controlling.value || input.mode.value === 'PARK')
const neckDeg = computed(() => ({
  yaw: ((input.neckPose.yaw * 180) / Math.PI).toFixed(0),
  pitch: ((input.neckPose.pitch * 180) / Math.PI).toFixed(0),
}))

function selectGear(gear: SpeedGear) {
  saveClientConfig({ speed_gear: gear })
}

function toggleListen() {
  saveClientConfig({ mic_listen_enabled: !clientConfig.mic_listen_enabled })
}

function onStopClick() {
  input.stopAllInputs()
  wan.emergencyStop()
}
</script>

<template>
  <div class="control-deck">
    <dv-border-box-10 :color="['#00d8ff', '#1d6fff']">
      <div class="deck-inner">
        <!-- 1. 急停与失权解锁区（连上已自动接管，仅失权/锁存时显露解锁键） -->
        <div class="deck-group safety-group">
          <div class="group-header">
            <span class="group-title">总控</span>
            <span class="group-header-line" />
          </div>
          <div class="group-body safety-body">
            <button
              v-if="wan.online.value && !wan.controlling.value"
              class="dv-btn dv-btn-sm dv-btn-ok recover-btn"
              :disabled="wan.busy.value"
              @click="wan.acquire(false)"
            >
              <template v-if="latched"><Unlock :size="14" /> 解锁控制</template>
              <template v-else><Hand :size="14" /> 恢复控制</template>
            </button>

            <button
              class="dv-btn dv-btn-danger stop-btn"
              :class="{ 'pulse-danger': wan.online.value }"
              :disabled="!wan.online.value"
              title="空格键也可触发急停"
              @click="onStopClick"
            >
              <OctagonX :size="20" /> 急停
            </button>
          </div>
        </div>

<!--         <div class="deck-divider" aria-hidden="true">
          <span class="divider-node top" />
          <span class="divider-line" />
          <span class="divider-node bottom" />
        </div> -->

        <!-- 2. 底盘操控：模式 + 三挡速度 + 回中圆盘摇杆 -->
        <div class="deck-group chassis-group">
          <div class="group-header">
            <span class="group-title">底盘</span>
            <span class="group-header-line" />
          </div>
          <div class="group-body chassis-body">
            <div class="sub-col">
              <div class="mode-grid">
                <button
                  v-for="m in modes"
                  :key="m.value"
                  class="dv-btn dv-btn-sm"
                  :class="{ active: input.mode.value === m.value }"
                  :disabled="!wan.controlling.value"
                  @click="input.setChassisMode(m.value)"
                >
                  {{ m.label }}
                </button>
              </div>
              <div class="gear-grid">
                <button
                  v-for="g in gears"
                  :key="g.value"
                  class="dv-btn dv-btn-sm"
                  :class="{ active: clientConfig.speed_gear === g.value }"
                  @click="selectGear(g.value)"
                >
                  {{ g.label }}
                </button>
              </div>
              <div class="readout-line mono">
                <span>{{ input.chassisOutput.value.speed.toFixed(2) }} m/s</span>
                <span>
                  {{ input.mode.value === 'SPIN'
                    ? `${input.chassisOutput.value.yaw.toFixed(2)} r/s`
                    : `${((input.chassisOutput.value.steer * 180) / Math.PI).toFixed(0)}°` }}
                </span>
              </div>
            </div>

            <JoystickPad
              :size="96"
              :spring="true"
              :model-x="input.driveStick.value.x" :model-y="input.driveStick.value.y"
              :lock-y="input.mode.value === 'SPIN'"
              :disabled="parkLocked"
              :disabled-label="input.mode.value === 'PARK' ? '驻车' : ''"
              top-label="前进"
              @change="input.setButtonDrive"
            />
          </div>
        </div>

<!--         <div class="deck-divider" aria-hidden="true">
          <span class="divider-node top" />
          <span class="divider-line" />
          <span class="divider-node bottom" />
        </div>
 -->
        <!-- 3. 头部操控：非回中圆盘摇杆（位置直映） + 回中 -->
        <div class="deck-group head-group">
          <div class="group-header">
            <span class="group-title">头部</span>
            <span class="group-header-line" />
          </div>
          <div class="group-body head-body">
            <JoystickPad
              :size="96"
              :spring="false"
              :disabled="input.headDisabled.value" :disabled-label="input.headStatus.value"
              :model-x="input.neckStick.x"
              :model-y="input.neckStick.y"
              top-label="抬头"
              @change="input.setNeckStick"
            />

            <div class="head-side">
              <div class="head-readout mono">
                <div><span>偏航</span><strong>{{ neckDeg.yaw }}°</strong></div>
                <div><span>俯仰</span><strong>{{ neckDeg.pitch }}°</strong></div>
              </div>
              <button
                class="dv-btn dv-btn-sm"
                :disabled="!input.canCenterHead.value"
                @click="input.centerHead()"
              >
                <RotateCcw :size="13" /> 回中
              </button>
            </div>
          </div>
        </div>

        <!-- <div class="deck-divider" aria-hidden="true">
          <span class="divider-node top" />
          <span class="divider-line" />
          <span class="divider-node bottom" />
        </div> -->

        <!-- 4. 语音对讲与监听 -->
        <div class="deck-group voice-group">
          <div class="group-header">
            <span class="group-title">语音</span>
            <span class="group-header-line" />
          </div>
          <div class="group-body voice-body">
            <button
              class="dv-btn talk-btn"
              :class="wan.talking.value ? 'dv-btn-danger pulse-danger' : 'dv-btn-ok'"
              :disabled="!wan.online.value || !wan.micAvailable.value"
              title="按住讲话（键盘 ` / ~）"
              @pointerdown.prevent="wan.holdTalk('pointer')"
              @pointerup="wan.releaseTalk('pointer')"
              @pointerleave="wan.releaseTalk('pointer')"
              @pointercancel="wan.releaseTalk('pointer')"
            >
              <component :is="wan.talking.value ? Mic : MicOff" :size="16" />
              {{ wan.talking.value ? '讲话中…' : '按住讲话' }}
            </button>

            <button
              class="dv-btn dv-btn-sm"
              :class="{ active: clientConfig.mic_listen_enabled }"
              @click="toggleListen"
            >
              <component :is="clientConfig.mic_listen_enabled ? Volume2 : VolumeX" :size="13" />
              {{ clientConfig.mic_listen_enabled ? '监听开' : '监听关' }}
            </button>
          </div>
        </div>

        <VoiceDialog />
        <SpeakerPanel class="deck-group sound-group" />
        <div class="corner-config">
          <button class="dv-btn dv-btn-sm config-btn" title="打开配置后台" @click="emit('openConfig')">
            <Settings :size="14" /> 配置
          </button>
        </div>
      </div>
    </dv-border-box-10>
  </div>
</template>

<style scoped>
.control-deck {
  width: 100%;
  height: 100%;
}

.deck-inner {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 6px;
  height: 100%;
  padding: 8px 14px;
}

.deck-group {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 6px;
  padding: 6px 10px 8px;
  border: 1px solid rgba(0, 216, 255, 0.2);
  border-radius: 4px;
  background: rgba(5, 20, 48, 0.62);
  height: 100%;
}

.group-header {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 18px;
  flex-shrink: 0;
}

.group-title {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.14em;
  color: var(--cyan-bright);
  text-shadow: 0 0 8px rgba(0, 216, 255, 0.6);
  white-space: nowrap;
}

.group-title::before {
  content: '';
  display: inline-block;
  width: 4px;
  height: 11px;
  background: linear-gradient(180deg, #67f3ff, #1d6fff);
  transform: skewX(-18deg);
  box-shadow: 0 0 6px rgba(0, 216, 255, 0.8);
}

.group-header-line {
  position: relative;
  flex: 1;
  height: 1px;
  background: linear-gradient(
    90deg,
    rgba(0, 216, 255, 0.75) 0%,
    rgba(29, 111, 255, 0.3) 60%,
    transparent 100%
  );
}

.group-body {
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: center;
  gap: 10px;
}

.safety-group {
  min-width: 112px;
}

.safety-body,
.voice-body {
  flex-direction: column;
  justify-content: center;
  gap: 6px;
}

.recover-btn,
.stop-btn {
  width: 100%;
}

.stop-btn {
  flex: 1;
  font-size: 16px;
  letter-spacing: 0.12em;
}

.sub-col {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 6px;
  width: 168px;
}

.mode-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 4px;
}

.gear-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 4px;
}

.readout-line {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--cyan-bright);
  padding: 0 2px;
}

.head-side {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 8px;
  min-width: 78px;
  height: 92px;
}

.head-readout {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 11px;
}

.head-readout div {
  display: flex;
  justify-content: space-between;
  gap: 6px;
}

.head-readout span {
  color: var(--text-dim);
}

.head-readout strong {
  color: var(--cyan-bright);
}

.voice-group {
  min-width: 116px;
}

.voice-body .dv-btn {
  width: 100%;
}


.talk-btn {
  flex: 1;
  min-height: 44px;
}

.deck-divider {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 12px;
  height: 84%;
  flex-shrink: 0;
  gap: 3px;
}

.divider-line {
  width: 1px;
  flex: 1;
  background: linear-gradient(
    180deg,
    transparent 0%,
    rgba(0, 216, 255, 0.85) 25%,
    rgba(29, 111, 255, 0.85) 50%,
    rgba(0, 216, 255, 0.85) 75%,
    transparent 100%
  );
  box-shadow: 0 0 6px rgba(0, 216, 255, 0.45);
}

.divider-node {
  width: 5px;
  height: 5px;
  background: var(--cyan-bright);
  transform: rotate(45deg);
  box-shadow: 0 0 6px rgba(103, 243, 255, 0.9);
  flex-shrink: 0;
}

.sound-group { flex: 0 1 230px; min-width: 158px; }

.corner-config {
  margin-left: auto;
  display: flex;
  align-items: flex-end;
  height: 100%;
}

.config-btn {
  padding: 6px 10px;
  border-color: rgba(103, 243, 255, 0.55);
}
</style>
