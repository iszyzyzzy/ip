<script setup lang="ts">
import { computed } from 'vue'
import { useWanClient } from '@/lib/wan-client'
import { interpolateColor } from '@/lib/color'
import { describeNetwork } from '@/lib/network'

import Progress from './Progress.vue'

const wan = useWanClient()

const battery = computed(() => {
  const b = wan.state.value?.devices?.chassis?.battery
  const sys = wan.state.value?.devices?.chassis?.system
  const soc = typeof b?.soc === 'number' ? Math.max(0, Math.min(100, Math.round(b.soc))) : null
  const voltage = b?.voltage_v ?? sys?.voltage_v ?? null
  const tempNum = typeof b?.temperature_c === 'number' ? b.temperature_c : null
  return {
    soc,
    tempNum,
    voltage: typeof voltage === 'number' ? voltage.toFixed(1) : '—',
    current: typeof b?.current_a === 'number' ? b.current_a.toFixed(1) : '—',
    temp: tempNum != null ? `${tempNum.toFixed(0)}℃` : '—',
  }
})

const network = computed(() => {
  const stats = wan.rtcStats.value
  return {
    candidateType: stats.candidateType,
    rttMs: stats.rttMs,
  }
})

const carSignal = computed(() => describeNetwork(wan.state.value?.network))

const batteryColor = computed(() =>
  battery.value.soc == null
    ? '#00b8ff'
    : interpolateColor(battery.value.soc, '#ff3b53', '#18e69a'),
)

const networkColor = computed(() =>
  interpolateColor(
    ((network.value.rttMs ?? 0) / 1000) * 100,
    '#18e69a',
    '#ff3b53',
  ),
)

const tempColor = computed(() =>
  battery.value.tempNum == null
    ? '#00b8ff'
    : interpolateColor(
        battery.value.tempNum,
        '#18e69a',
        '#ff3b53',
      ),
)

const signalColor = computed(() =>
  carSignal.value.percent == null
    ? '#00b8ff'
    : interpolateColor(carSignal.value.percent, '#ff3b53', '#18e69a'),
)

</script>

<template>
  <div class="capsule-chart" >
    <div class="capsule-chart-item">
      <span class="capsule-chart-label">电量</span>
      <Progress :value="battery.soc ?? 0" :max="100" :color="batteryColor" 
        trackColor="rgba(0, 216, 255, 0.12)" :height="10" animated label="电量" />
      <span class="capsule-chart-value">{{ battery.soc ?? '—' }}%</span>
    </div>
    <div class="capsule-chart-item">
      <span class="capsule-chart-label">温度</span>
      <Progress :value="battery.tempNum ?? 0" :max="80" :color="tempColor" 
        trackColor="rgba(0, 216, 255, 0.12)" :height="10" animated label="温度" />
      <span class="capsule-chart-value">{{ battery.temp }}</span>
    </div>
    <div class="capsule-chart-item">
      <span class="capsule-chart-label">延迟</span>
      <Progress :value="network.rttMs ?? 0" :max="1000" :color="networkColor" 
        trackColor="rgba(0, 216, 255, 0.12)" :height="10" animated label="延迟" />
      <span class="capsule-chart-value">{{ network.rttMs ?? '—' }}ms</span>
    </div>
    <div class="capsule-chart-item" :title="carSignal.summary">
      <span class="capsule-chart-label">信号</span>
      <Progress :value="carSignal.percent ?? 0" :max="100" :color="signalColor" 
        trackColor="rgba(0, 216, 255, 0.12)" :height="10" animated label="信号" />
      <span class="capsule-chart-value">{{ carSignal.percent ?? '—' }}%</span>
    </div>
  </div>
</template>

<style scoped>
.capsule-chart {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr) max-content;
  grid-auto-flow: row;
  grid-auto-rows: 1fr;
  height: 100%;
  width: 100%;
  min-width: 0;
}
.capsule-chart-item {
  display: grid;
  grid-column: 1 / -1;
  grid-template-columns: subgrid;
  align-items: center;
  min-width: 0;
}
.capsule-chart-label {
  text-align: right;
  margin-right: 8px;
  color: var(--text-dim);
  font-size: 12px;
  white-space: nowrap;
}
.capsule-chart-item :deep(.capsule-progress) {
  min-width: 0;
}
.capsule-chart-value {
  margin-left: 8px;
  color: var(--cyan-bright);
  font-size: 12px;
  white-space: nowrap;
}
</style>