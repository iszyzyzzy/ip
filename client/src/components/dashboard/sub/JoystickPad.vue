<script setup lang="ts">
import { computed, ref } from 'vue'

const props = withDefaults(defineProps<{
  size?: number
  spring?: boolean
  lockY?: boolean
  disabled?: boolean
  modelX?: number
  modelY?: number
  topLabel?: string
  disabledLabel?: string
}>(), {
  size: 96,
  spring: true,
  lockY: false,
  disabled: false,
  modelX: 0,
  modelY: 0,
  topLabel: '',
  disabledLabel: '',
})

const emit = defineEmits<{
  change: [x: number, y: number]
  end: []
}>()

const padEl = ref<HTMLDivElement | null>(null)
const dragging = ref(false)
const knobStyle = computed(() => {
  const x = props.disabled && props.spring ? 0 : props.modelX
  const y = props.lockY || (props.disabled && props.spring) ? 0 : props.modelY
  return {
    left: `${50 + x * 36}%`,
    top: `${50 - y * 36}%`,
  }
})

function updateFromPointer(e: PointerEvent) {
  if (!padEl.value || props.disabled) return
  const rect = padEl.value.getBoundingClientRect()
  const cx = rect.left + rect.width / 2
  const cy = rect.top + rect.height / 2
  const maxR = Math.max(12, rect.width / 2 - 14)
  let x = (e.clientX - cx) / maxR
  let y = props.lockY ? 0 : -(e.clientY - cy) / maxR
  const len = Math.hypot(x, y)
  if (len > 1) {
    x /= len
    y /= len
  }
  const rx = Math.round(x * 1000) / 1000
  const ry = Math.round(y * 1000) / 1000
  emit('change', rx, ry)
}

function onPointerDown(e: PointerEvent) {
  if (props.disabled) return
  dragging.value = true
  ;(e.currentTarget as Element | null)?.setPointerCapture?.(e.pointerId)
  updateFromPointer(e)
}

function onPointerMove(e: PointerEvent) {
  if (!dragging.value || props.disabled) return
  updateFromPointer(e)
}

function onPointerEnd() {
  if (!dragging.value) return
  dragging.value = false
  if (props.spring) {
    emit('change', 0, 0)
  }
  emit('end')
}
</script>

<template>
  <div
    ref="padEl"
    class="joystick-pad"
    :class="{ disabled }"
    :style="{ width: `${size}px`, height: `${size}px` }"
    @pointerdown.prevent="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerEnd"
    @pointercancel="onPointerEnd"
  >
    <div class="axis-x" />
    <div v-if="!lockY" class="axis-y" />
    <span v-if="topLabel && !disabled" class="top-label">{{ topLabel }}</span>
    <span v-if="disabled && disabledLabel" class="disabled-label">{{ disabledLabel }}</span>
    <div class="knob" :class="{ active: dragging }" :style="knobStyle" />
  </div>
</template>

<style scoped>
.joystick-pad {
  position: relative;
  border-radius: 50%;
  border: 1.5px solid rgba(0, 216, 255, 0.52);
  background: radial-gradient(circle, rgba(0, 140, 255, 0.16) 0%, rgba(3, 12, 32, 0.92) 72%);
  box-shadow: inset 0 0 14px rgba(0, 216, 255, 0.18), 0 0 10px rgba(0, 140, 255, 0.14);
  touch-action: none;
  cursor: pointer;
  flex-shrink: 0;
}

.joystick-pad.disabled {
  border-color: rgba(124, 142, 162, 0.38);
  background: radial-gradient(circle, rgba(90, 104, 120, 0.14) 0%, rgba(16, 24, 36, 0.88) 72%);
  box-shadow: none;
  cursor: not-allowed;
}

.axis-x,
.axis-y {
  position: absolute;
  pointer-events: none;
  background: rgba(0, 216, 255, 0.16);
}

.joystick-pad.disabled .axis-x,
.joystick-pad.disabled .axis-y {
  background: rgba(140, 154, 170, 0.16);
}

.axis-x {
  left: 12%;
  right: 12%;
  top: 50%;
  height: 1px;
}

.axis-y {
  top: 12%;
  bottom: 12%;
  left: 50%;
  width: 1px;
}

.top-label {
  position: absolute;
  top: 4px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 10px;
  color: var(--text-dim);
  pointer-events: none;
}

.disabled-label {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 600;
  color: #8a99ad;
  pointer-events: none;
  z-index: 1;
}

.knob {
  position: absolute;
  width: 24px;
  height: 24px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  border: 1.5px solid #bdf7ff;
  background: radial-gradient(circle at 35% 35%, #67f3ff, #0077cc);
  box-shadow: 0 0 10px rgba(0, 216, 255, 0.65);
  pointer-events: none;
}

.knob.active {
  box-shadow: 0 0 16px rgba(103, 243, 255, 0.95);
}

.joystick-pad.disabled .knob {
  border-color: rgba(160, 172, 188, 0.45);
  background: radial-gradient(circle at 35% 35%, #8b97a6, #4b5563);
  box-shadow: none;
  opacity: 0.45;
}
</style>
