<script setup lang="ts">
import { computed } from 'vue'
import { describeNetwork } from '@/lib/network'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()

const summary = computed(() => {
  const s = wan.state.value
  const d = s?.devices ?? {}
  const snap = wan.snapshot.value
  const stats = wan.rtcStats.value
  const net = s?.network
  const netDesc = describeNetwork(net)
  return [
    ['运行模式', s ? (s.simulated ? '模拟仿真' : '实机硬件') : '—'],
    ['状态版本号', s?.revision ?? '—'],
    ['配置 / 已应用版本', s ? `${s.config_revision} / ${s.applied_config_revision}` : '—'],
    ['当前控制者', s?.owner || '无'],
    ['租约剩余', s?.owner ? `${s.lease_remaining_ms} ms` : '—'],
    ['停止 / 锁存', s ? `${s.stopped ? '已停止' : '运行中'} / ${s.latched ? '已锁定' : '未锁定'}` : '—'],
    ['H7 固件', d.h7?.firmware ? `${d.h7.firmware} (${d.h7.board || ''})` : '—'],
    ['ICE 路径 / RTT', `${stats.candidateType} / ${stats.rttMs != null ? stats.rttMs + ' ms' : '—'}`],
    ['车端网络 / 网卡', net ? `${netDesc.kindLabel} · ${net.interface || '—'} · ${net.connected ? '已连' : '断开'}` : '—'],
    ['车端信号强度', net ? [netDesc.percent != null ? `${netDesc.percent}%` : '—', netDesc.dbmText, net.technology].filter(Boolean).join(' · ') : '—'],
    ['SSID / 运营商', net ? (netDesc.identity || net.error || '—') : '—'],
    ['视频帧率 / 码率', `${stats.videoFps ?? 0} FPS / ${stats.bitrateKbps ?? 0} kbps`],
    ['丢弃底盘指令计数', String(snap?.dropped_drive ?? 0)],
    ['喊话通道阶段', snap?.talk ? `${snap.talk.phase} ${snap.talk.message || ''}` : '—'],
  ]
})

const events = computed(() => (wan.state.value?.devices?.events ?? []).slice(-12).reverse())
</script>

<template>
  <div class="diag-root dv-scroll">
    <section class="dv-card diag-card">
      <div class="dv-section-title">
        <span>整车遥测与通信诊断</span>
      </div>
      <div class="kv-grid mono">
        <div v-for="[k, v] in summary" :key="k" class="kv-item">
          <span class="k">{{ k }}</span>
          <span class="v">{{ v }}</span>
        </div>
      </div>
    </section>

    <section v-if="events.length" class="dv-card diag-card">
      <div class="dv-section-title">
        <span>最近控制事件</span>
      </div>
      <ul class="evt-list mono">
        <li v-for="(e, idx) in events" :key="idx">
          {{ e.operation }} {{ e.value ? JSON.stringify(e.value) : '' }}
        </li>
      </ul>
    </section>

    <section class="dv-card diag-card">
      <div class="dv-section-title">
        <span>state.v1 原始状态快照</span>
      </div>
      <pre class="json-pre mono">{{ JSON.stringify(wan.snapshot.value, null, 2) }}</pre>
    </section>
  </div>
</template>

<style scoped>
.diag-root {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding-right: 4px;
}

.diag-card {
  padding: 12px;
}

.kv-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.kv-item {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  padding: 6px 8px;
  border: 1px solid rgba(0, 216, 255, 0.16);
  background: rgba(4, 16, 40, 0.72);
  border-radius: 3px;
  font-size: 12px;
}

.kv-item .k { color: var(--text-dim); }
.kv-item .v { color: var(--cyan-bright); text-align: right; }

.evt-list {
  margin: 0;
  padding-left: 18px;
  font-size: 11px;
  color: var(--text-main);
  line-height: 1.6;
}

.json-pre {
  margin: 0;
  max-height: 300px;
  overflow: auto;
  padding: 10px;
  border-radius: 3px;
  background: rgba(2, 8, 22, 0.92);
  border: 1px solid rgba(0, 216, 255, 0.2);
  font-size: 11px;
  color: #a9e7ff;
  user-select: text;
}
</style>
