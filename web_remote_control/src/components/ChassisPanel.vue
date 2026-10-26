<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import Joystick from '@/components/Joystick.vue'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { type ChassisMode, useRemote } from '@/lib/remote'

const props = defineProps<{ config: Record<string, any> | null }>()
const remote = useRemote()

// 与 robotd validation 一致的官方上限；数据库限值更小时以数据库为准。
const OFFICIAL_STEER = { ACKERMANN: 0.698, CRAB: 1.571 }
const STEER_20_DEG = (20 * Math.PI) / 180
const SPEED_AT_WIDE_STEER = 0.525
const SEND_MS = 50
const TTL_MS = 200

const mode = ref<ChassisMode>('ACKERMANN')
const scale = ref([50])
const stick = { x: 0, y: 0 }
const output = ref({ speed: 0, steer: 0, yaw: 0 })
let timer = 0

const limits = computed(() => {
  const c = props.config?.chassis ?? {}
  return {
    speed: c.max_speed_m_s ?? 0.3,
    yaw: c.max_yaw_rate_rad_s ?? 0.5,
    steer: c.max_steering_rad ?? 0.698,
  }
})

const modes: { value: ChassisMode, label: string }[] = [
  { value: 'ACKERMANN', label: '阿克曼' },
  { value: 'CRAB', label: '斜移' },
  { value: 'SPIN', label: '自旋' },
  { value: 'PARK', label: '驻车' },
]

function compute() {
  const k = scale.value[0]! / 100
  const lim = limits.value
  let speed = 0
  let steer = 0
  let yaw = 0
  if (mode.value === 'SPIN') {
    // 协议规定逆时针为正；摇杆向左推即逆时针。
    yaw = -stick.x * lim.yaw * k
  } else if (mode.value !== 'PARK') {
    speed = stick.y * lim.speed * k
    // 协议规定左转为正；转角不随档位缩放，便于低速时也能打满方向。
    const maxSteer = Math.min(lim.steer, OFFICIAL_STEER[mode.value])
    steer = -stick.x * maxSteer
    if (Math.abs(steer) > STEER_20_DEG)
      speed = Math.max(-SPEED_AT_WIDE_STEER, Math.min(SPEED_AT_WIDE_STEER, speed))
  }
  const round = (v: number) => Math.round(v * 1000) / 1000
  output.value = { speed: round(speed), steer: round(steer), yaw: round(yaw) }
}

function chassisTarget() {
  const { speed, steer, yaw } = output.value
  return {
    chassis: {
      mode: mode.value,
      speed_m_s: speed,
      steering_rad: steer,
      yaw_rate_rad_s: yaw,
      ttl_ms: TTL_MS,
    },
  }
}

function startStream() {
  clearInterval(timer)
  // 按住期间持续发送：底盘目标 TTL 很短，只有新指令才能维持运动。
  timer = window.setInterval(() => remote.targets(chassisTarget()), SEND_MS)
}

function stopStream() {
  clearInterval(timer)
  timer = 0
  stick.x = 0
  stick.y = 0
  compute()
  // 松手立即补发零速度，而不是等 TTL 到期才停。
  remote.targets(chassisTarget(), true)
}

function onMove(x: number, y: number) {
  stick.x = x
  stick.y = y
  compute()
}

async function selectMode(value: unknown) {
  if (!value) return
  mode.value = value as ChassisMode
  stick.x = 0
  stick.y = 0
  compute()
  // 先下发零速度的新模式，让底盘提前切换，而非等到第一次推杆。
  await remote.targets(chassisTarget(), true)
}

watch(() => remote.controlling.value, (on) => {
  if (!on) clearInterval(timer)
})
onBeforeUnmount(() => {
  if (timer) stopStream()
})

const ready = computed(() => remote.state.value?.devices?.chassis?.ready)
</script>

<template>
  <div class="flex flex-col gap-4">
    <ToggleGroup
      type="single"
      variant="outline"
      :model-value="mode"
      class="grid w-full grid-cols-4"
      @update:model-value="selectMode"
    >
      <ToggleGroupItem
        v-for="m in modes"
        :key="m.value"
        :value="m.value"
        class="h-11 text-sm"
        :disabled="!remote.controlling.value"
      >
        {{ m.label }}
      </ToggleGroupItem>
    </ToggleGroup>

    <div class="mx-auto w-full max-w-[min(80vw,22rem)]">
      <Joystick
        label="底盘摇杆"
        :disabled="!remote.controlling.value || mode === 'PARK'"
        :lock-y="mode === 'SPIN'"
        @start="startStream"
        @move="onMove"
        @end="stopStream"
      >
        <span class="pointer-events-none absolute left-1/2 top-3 -translate-x-1/2 text-xs text-muted-foreground">
          {{ mode === 'SPIN' ? '左右自旋' : '前进' }}
        </span>
      </Joystick>
    </div>

    <div class="grid grid-cols-3 gap-2 text-center tabular-nums">
      <div class="rounded-lg bg-muted/50 p-2">
        <div class="text-xs text-muted-foreground">速度 m/s</div>
        <div class="text-lg font-semibold">{{ output.speed.toFixed(2) }}</div>
      </div>
      <div class="rounded-lg bg-muted/50 p-2">
        <div class="text-xs text-muted-foreground">转角 °</div>
        <div class="text-lg font-semibold">{{ (output.steer * 180 / Math.PI).toFixed(0) }}</div>
      </div>
      <div class="rounded-lg bg-muted/50 p-2">
        <div class="text-xs text-muted-foreground">自旋 rad/s</div>
        <div class="text-lg font-semibold">{{ output.yaw.toFixed(2) }}</div>
      </div>
    </div>

    <div class="flex items-center gap-3">
      <span class="w-14 shrink-0 text-sm text-muted-foreground">档位</span>
      <Slider v-model="scale" :min="10" :max="100" :step="10" class="py-3" @update:model-value="compute" />
      <span class="w-12 shrink-0 text-right text-sm tabular-nums">{{ scale[0] }}%</span>
    </div>
    <p class="text-xs text-muted-foreground">
      上限 {{ limits.speed }} m/s · {{ limits.yaw }} rad/s
      <span v-if="ready === false" class="text-destructive"> · 底盘未就绪，仅发送零速度</span>
    </p>

    <Button
      v-if="mode === 'PARK'"
      variant="outline"
      class="h-11"
      :disabled="!remote.controlling.value"
      @click="selectMode('PARK')"
    >
      重新发送驻车
    </Button>
  </div>
</template>
