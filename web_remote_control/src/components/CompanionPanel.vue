<script setup lang="ts">
import { computed } from 'vue'
import OfflinePanel from './OfflinePanel.vue'
import { useRemote } from '@/lib/remote'

const props = defineProps<{ chassisOnline: boolean, live: boolean }>()
const remote = useRemote()
const state = computed(() => props.live ? remote.state.value : null)
const pad = computed(() => props.live ? remote.gamepad.value : null)
const chassis = computed(() => state.value?.devices?.chassis)
const motion = computed(() => chassis.value?.feedback_age_ms?.motion < 1000 ? chassis.value?.motion : null)
const speed = computed(() => motion.value?.speed_m_s?.toFixed(2) ?? '—')
const steer = computed(() => motion.value ? Math.round(motion.value.steering_raw_rad * 180 / Math.PI) + '°' : '—')
</script>

<template>
  <div class="instrument-grid grid gap-3">
    <OfflinePanel name="底盘" :offline="!chassisOnline">
      <section class="h-full rounded-xl border p-4">
        <h2 class="text-sm font-medium">底盘反馈</h2>
        <div class="my-2 flex items-baseline gap-2">
          <strong class="text-5xl font-semibold tabular-nums">{{ speed }}</strong><span>m/s</span>
        </div>
        <div class="flex flex-wrap gap-x-5 gap-y-1 text-sm text-muted-foreground">
          <span>转向 {{ steer }}</span><span>电量 {{ chassis?.battery?.soc != null ? chassis.battery.soc + '%' : '—' }}</span>
        </div>
      </section>
    </OfflinePanel>
    <section class="rounded-xl border p-4">
      <div class="flex flex-wrap items-baseline justify-between gap-2">
        <h2 class="text-sm font-medium">{{ pad?.device || '手柄未连接' }}</h2>
        <span class="text-xs text-muted-foreground">{{ pad?.controlling ? '手柄控制中' : state?.owner || '等待接管' }}</span>
      </div>
      <p class="my-2 text-lg font-semibold">{{ state?.action || '当前无动作' }}</p>
      <div v-if="Object.keys(pad?.bindings ?? {}).length" class="grid grid-cols-2 gap-2">
        <div v-for="(action, chord) in pad?.bindings" :key="chord" class="rounded-md bg-muted px-2 py-1 text-sm"
          :class="{ 'ring-2 ring-primary': String(chord).split('+').every(k => pad?.held.includes(k)) }">
          <kbd class="mr-1 text-xs">{{ chord }}</kbd>{{ action }}
        </div>
      </div>
      <p v-else class="text-sm text-muted-foreground">在全控遥控的「调试 → 手柄」中绑定动作。</p>
      <p v-if="pad?.error" role="status" class="mt-2 text-sm text-destructive">{{ pad.error }}</p>
    </section>
  </div>
  <p class="text-center text-xs text-muted-foreground">Menu 接管 / 释放 · LB + 左摇杆驾驶 · View 急停</p>
</template>

<style scoped>
@media (orientation: landscape) and (min-width: 520px) {
  .instrument-grid { grid-template-columns: 0.9fr 1.1fr; }
}
</style>
