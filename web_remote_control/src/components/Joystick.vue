<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { cn } from '@/lib/utils'

// 输出归一化坐标：x 向右为正，y 向上为正，范围 -1..1。
const props = withDefaults(defineProps<{
  disabled?: boolean
  // 弹回：松手归零，用于速度；保持：松手停在原处，用于颈部位置。
  spring?: boolean
  lockX?: boolean
  lockY?: boolean
  label?: string
  class?: string
}>(), { spring: true })

const emit = defineEmits<{
  move: [x: number, y: number]
  start: []
  end: []
}>()

const pad = ref<HTMLElement>()
const x = ref(0)
const y = ref(0)
const active = ref(false)
let pointerId: number | null = null

function update(event: PointerEvent) {
  const rect = pad.value!.getBoundingClientRect()
  const radius = rect.width / 2
  let dx = (event.clientX - rect.left - radius) / radius
  let dy = -(event.clientY - rect.top - radius) / radius
  if (props.lockX) dx = 0
  if (props.lockY) dy = 0
  const length = Math.hypot(dx, dy)
  if (length > 1) {
    dx /= length
    dy /= length
  }
  // 小死区避免手指接触抖动让机器人蠕动。
  const dead = (v: number) => (Math.abs(v) < 0.06 ? 0 : v)
  x.value = dead(dx)
  y.value = dead(dy)
  emit('move', x.value, y.value)
}

function down(event: PointerEvent) {
  if (props.disabled || pointerId !== null) return
  pointerId = event.pointerId
  pad.value!.setPointerCapture(event.pointerId)
  active.value = true
  navigator.vibrate?.(10)
  emit('start')
  update(event)
}

function move(event: PointerEvent) {
  if (event.pointerId === pointerId) update(event)
}

function up(event: PointerEvent) {
  if (event.pointerId !== pointerId) return
  pointerId = null
  active.value = false
  if (props.spring) {
    x.value = 0
    y.value = 0
    emit('move', 0, 0)
  }
  emit('end')
}

function reset(nx = 0, ny = 0) {
  x.value = nx
  y.value = ny
}

defineExpose({ reset })

onBeforeUnmount(() => {
  if (pointerId !== null) emit('end')
})
</script>

<template>
  <div
    ref="pad"
    role="application"
    :aria-label="label"
    :aria-disabled="disabled"
    :class="cn(
      'relative aspect-square w-full touch-none select-none rounded-full border-2 bg-muted/40 transition-colors',
      active ? 'border-primary bg-muted/70' : 'border-border',
      disabled && 'opacity-40',
      props.class,
    )"
    @pointerdown="down"
    @pointermove="move"
    @pointerup="up"
    @pointercancel="up"
    @lostpointercapture="up"
  >
    <!-- 十字参考线，帮助判断方向与中位。 -->
    <div class="pointer-events-none absolute inset-x-6 top-1/2 h-px bg-border" />
    <div class="pointer-events-none absolute inset-y-6 left-1/2 w-px bg-border" />
    <slot />
    <div
      class="pointer-events-none absolute left-1/2 top-1/2 size-[30%] rounded-full border-2 border-primary/60 bg-primary/90 shadow-lg"
      :class="active ? '' : 'transition-transform duration-150'"
      :style="{ transform: `translate(calc(-50% + ${x * 117}%), calc(-50% + ${-y * 117}%))` }"
    />
  </div>
</template>
