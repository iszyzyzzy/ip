<script setup lang="ts">
import { computed, ref } from 'vue'
import Joystick from '@/components/Joystick.vue'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { useRemote } from '@/lib/remote'

const props = defineProps<{ config: Record<string, any> | null }>()
const remote = useRemote()
const pad = ref<InstanceType<typeof Joystick>>()
const eyesFollow = ref(true)
const pose = ref({ yaw: 0, pitch: 0 })

// 颈部是位置目标且由 H7 闭环；节流到 20 Hz 足够跟手，也不会挤占底盘指令。
const SEND_MS = 50
let lastSent = 0
let trailing = 0

const range = computed(() => {
  const n = props.config?.neck ?? {}
  return {
    yaw: [n.yaw?.min_rad ?? -0.5, n.yaw?.max_rad ?? 0.5] as const,
    pitch: [n.pitch?.min_rad ?? -0.5, n.pitch?.max_rad ?? 0.5] as const,
  }
})

// 摇杆 -1..1 映射到配置限位两端，保证不会请求超出结构限位的位置。
function map(v: number, [lo, hi]: readonly [number, number]) {
  return v >= 0 ? v * hi : -v * lo
}

function payload(x: number, y: number) {
  // 偏航以向左为正，与底盘一致；摇杆向右即负偏航。
  const yaw = map(-x, range.value.yaw)
  const pitch = map(y, range.value.pitch)
  pose.value = { yaw, pitch }
  const targets: Record<string, unknown> = { neck: { yaw_rad: yaw, pitch_rad: pitch } }
  if (eyesFollow.value) {
    const eye = { gaze_x: x * 0.6, gaze_y: -y * 0.6, openness: 1, brow: 0 }
    targets.eyes = { left: eye, right: eye, roll: 0 }
  }
  return targets
}

function onMove(x: number, y: number) {
  const now = Date.now()
  clearTimeout(trailing)
  if (now - lastSent >= SEND_MS) {
    lastSent = now
    remote.targets(payload(x, y))
  } else {
    // 保证最后一个位置一定发出，否则松手位置可能与显示不一致。
    trailing = window.setTimeout(() => onMove(x, y), SEND_MS - (now - lastSent))
  }
}

function center() {
  clearTimeout(trailing)
  pad.value?.reset()
  remote.targets(payload(0, 0), true)
}

function blink(left: boolean, right: boolean) {
  remote.targets({ blink: { left, right } }, true)
}

async function zero() {
  if (!confirm('以当前颈部位置作为运行时零点？此操作会先停止运动。')) return
  await remote.command('zero_neck', { mask: 3 }).catch(() => {})
}

const deg = (v: number) => ((v * 180) / Math.PI).toFixed(0)
const feedback = computed(() => {
  const neck = remote.state.value?.devices?.neck
  if (!neck) return null
  return { yaw: neck.gm6020?.joint_position_rad, pitch: neck.dm4310?.joint_position_rad }
})
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="mx-auto w-full max-w-[min(80vw,22rem)]">
      <Joystick
        ref="pad"
        label="头部摇杆"
        :spring="false"
        :disabled="!remote.controlling.value"
        @move="onMove"
      >
        <span class="pointer-events-none absolute left-1/2 top-3 -translate-x-1/2 text-xs text-muted-foreground">抬头</span>
      </Joystick>
    </div>

    <div class="grid grid-cols-2 gap-2 text-center tabular-nums">
      <div class="rounded-lg bg-muted/50 p-2">
        <div class="text-xs text-muted-foreground">偏航 °</div>
        <div class="text-lg font-semibold">{{ deg(pose.yaw) }}</div>
        <div v-if="feedback?.yaw != null" class="text-xs text-muted-foreground">实际 {{ deg(feedback.yaw) }}</div>
      </div>
      <div class="rounded-lg bg-muted/50 p-2">
        <div class="text-xs text-muted-foreground">俯仰 °</div>
        <div class="text-lg font-semibold">{{ deg(pose.pitch) }}</div>
        <div v-if="feedback?.pitch != null" class="text-xs text-muted-foreground">实际 {{ deg(feedback.pitch) }}</div>
      </div>
    </div>

    <div class="flex items-center justify-between rounded-lg border p-3">
      <Label for="eyes-follow" class="text-sm">眼睛跟随摇杆</Label>
      <Switch id="eyes-follow" v-model="eyesFollow" />
    </div>

    <div class="grid grid-cols-3 gap-2">
      <Button variant="secondary" class="h-11" :disabled="!remote.controlling.value" @click="blink(true, true)">眨眼</Button>
      <Button variant="secondary" class="h-11" :disabled="!remote.controlling.value" @click="blink(false, true)">眨右眼</Button>
      <Button variant="secondary" class="h-11" :disabled="!remote.controlling.value" @click="blink(true, false)">眨左眼</Button>
    </div>
    <div class="grid grid-cols-2 gap-2">
      <Button variant="outline" class="h-11" :disabled="!remote.controlling.value" @click="center">回中</Button>
      <Button variant="outline" class="h-11" :disabled="!remote.controlling.value" @click="zero">设为零点</Button>
    </div>
  </div>
</template>
