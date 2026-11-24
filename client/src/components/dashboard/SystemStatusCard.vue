<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { describeNetwork } from '@/lib/network'
import { useWanClient } from '@/lib/wan-client'

import SystemCapsuleChart from './sub/SystemCapsuleChart.vue'

const wan = useWanClient()
const now = ref(Date.now())
let timer = 0

onMounted(() => {
  timer = window.setInterval(() => {
    now.value = Date.now()
  }, 500)
})
onUnmounted(() => clearInterval(timer))

const battery = computed(() => {
  const b = wan.state.value?.devices?.chassis?.battery
  const sys = wan.state.value?.devices?.chassis?.system
  const soc = typeof b?.soc === 'number' ? Math.max(0, Math.min(100, Math.round(b.soc))) : null
  const voltage = b?.voltage_v ?? sys?.voltage_v ?? null
  return {
    soc,
    voltage: typeof voltage === 'number' ? voltage.toFixed(1) : '—',
    current: typeof b?.current_a === 'number' ? b.current_a.toFixed(1) : '—',
    temp: typeof b?.temperature_c === 'number' ? `${b.temperature_c.toFixed(0)}℃` : '—',
  }
})

const batteryPondConfig = computed(() => {
  const value = battery.value.soc ?? 0
  const colors = value > 30 ? ['#00b8ff', '#18e69a'] : ['#ff3b53', '#ffb020']
  return {
    value,
    colors,
    lineDash: [4, 2],
    textColor: '#dcf6ff',
    formatter: battery.value.soc == null ? '未上报' : `${value}%`,
  }
})

const telemetryDelayMs = computed(() => {
  if (!wan.online.value || !wan.lastStateAt.value) return null
  return now.value - wan.lastStateAt.value
})

const rows = computed(() => {
  const s = wan.state.value
  const d = s?.devices ?? {}
  const snap = wan.snapshot.value
  const stats = wan.rtcStats.value
  const net = describeNetwork(s?.network)

  return [
    {
      label: '网络链路',
      value: wan.online.value
        ? `${stats.candidateType.toUpperCase()} · ${stats.rttMs != null ? stats.rttMs + ' ms' : '测速中'}`
        : '离线',
      tone: wan.online.value ? 'ok' : 'dim',
    },
/*     {
      label: '车端信号',
      value: wan.online.value ? net.summary : '—',
      tone: wan.online.value ? net.tone : 'dim',
    }, */
    {
      label: '遥测刷新',
      value: telemetryDelayMs.value == null ? '—' : `${telemetryDelayMs.value} ms`,
      tone: telemetryDelayMs.value != null && telemetryDelayMs.value > 1500 ? 'danger' : 'normal',
    },
    {
      label: '图传/拾音',
      value: `${snap?.video ?? '未启用'} / ${snap?.mic ?? '未启用'}`,
      tone: snap?.video === 'live' ? 'ok' : 'normal',
    },
    {
      label: '控制权/租约',
      value: s?.owner ? `${s.owner} (${s.lease_remaining_ms}ms)` : '空闲',
      tone: wan.controlling.value ? 'ok' : 'normal',
    },
    {
      label: '底盘/丢帧',
      value: `${d.chassis ? (d.chassis.ready ? '就绪' : '未就绪') : '—'} · 丢${snap?.dropped_drive ?? 0}`,
      tone: d.chassis?.ready === false ? 'warn' : 'normal',
    },
  ]
  /*
      {
      label: '偏航电机',
      value: d.neck?.gm6020 ? `${d.neck.gm6020.state} · ${d.neck.gm6020.temperature_c}℃` : '—',
      tone: d.neck?.gm6020?.fault && d.neck.gm6020.fault !== '无' ? 'danger' : 'normal',
    },
    {
      label: '俯仰电机',
      value: d.neck?.dm4310 ? `${d.neck.dm4310.state} · ${d.neck.dm4310.temperature_c}℃` : '—',
      tone: d.neck?.dm4310?.fault && d.neck.dm4310.fault !== '无' ? 'danger' : 'normal',
    },
  */
})

const alertText = computed(() => {
  if (wan.backendError.value) return `robotd 异常：${wan.backendError.value.message}`
  if (wan.state.value?.fault) return `设备故障：${wan.state.value.fault}`
  if (wan.state.value?.latched) return '急停锁定中，请确认安全后点击「解锁并控制」'
  return ''
})
</script>

<template>
  <div class="status-box dv-scroll">
    <div class="dv-section-title">
      <span>系统信息与能源</span>
<!--       <span class="dv-pill mono" :class="wan.online.value ? 'ok' : 'warn'">
        {{ wan.online.value ? '在线' : '未连' }}
      </span> -->
    </div>

    <div v-if="alertText" class="alert-banner">
      {{ alertText }}
    </div>

    <div class="info-card">
        <SystemCapsuleChart/>
      <!-- <dv-percent-pond :config="batteryPondConfig" class="pond" />
      <div class="battery-meta mono">
        <div><span>电压</span><strong>{{ battery.voltage }} V</strong></div>
        <div><span>电流</span><strong>{{ battery.current }} A</strong></div>
        <div><span>温度</span><strong>{{ battery.temp }}</strong></div>
      </div> -->
    </div>

    <div class="info-list">
      <div v-for="row in rows" :key="row.label" class="info-row">
        <span class="label">{{ row.label }}</span>
        <span class="val mono" :class="row.tone">{{ row.value }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.status-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
  flex: 1;
}

.info-card {
/*   display: grid;
  grid-template-columns: 110px 1fr; */
  gap: 8px;
  padding: 8px;
  border: 1px solid rgba(0, 216, 255, 0.25);
  border-radius: 4px;
  background: rgba(5, 22, 52, 0.72);
  align-items: center;
  height: 104px;
  width: 100%;
}

.pond {
  width: 110px;
  height: 54px;
}

.battery-meta {
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 11px;
}

.battery-meta div {
  display: flex;
  justify-content: space-between;
}

.battery-meta span {
  color: var(--text-dim);
}

.battery-meta strong {
  color: var(--cyan-bright);
}

.alert-banner {
  padding: 6px 8px;
  border: 1px solid rgba(255, 59, 83, 0.7);
  background: rgba(140, 12, 30, 0.45);
  border-radius: 3px;
  color: #ffb8c0;
  font-size: 12px;
  line-height: 1.4;
}

.info-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.info-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding: 5px 8px;
  border-radius: 3px;
  background: rgba(5, 20, 48, 0.56);
  border: 1px solid rgba(0, 216, 255, 0.14);
  font-size: 12px;
}

.label {
  color: var(--text-dim);
  white-space: nowrap;
}

.val {
  text-align: right;
  font-size: 11px;
  color: var(--text-main);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.val.ok { color: #7effcf; }
.val.warn { color: #ffd37d; }
.val.danger { color: #ff8e9b; }
.val.dim { color: var(--text-dim); }
</style>
