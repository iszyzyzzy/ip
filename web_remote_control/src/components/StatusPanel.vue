<script setup lang="ts">
import { computed } from 'vue'
import { useRemote } from '@/lib/remote'

const remote = useRemote()

const rows = computed(() => {
  const s = remote.state.value
  if (!s) return []
  const d = s.devices ?? {}
  const battery = d.chassis?.battery
  const system = d.chassis?.system
  const list: [string, string][] = [
    ['模式', s.simulated ? '模拟' : '实机'],
    ['控制者', s.owner || '无'],
    ['租约剩余', s.owner ? `${s.lease_remaining_ms} ms` : '—'],
    ['停止', s.stopped ? (s.latched ? '已锁定' : '是') : '否'],
    ['配置版本', `${s.config_revision}（已应用 ${s.applied_config_revision}）`],
    ['底盘就绪', d.chassis ? (d.chassis.ready ? '是' : '否') : '—'],
  ]
  if (battery) list.push(['电池', `${battery.soc}% · ${battery.voltage_v} V`])
  else if (system) list.push(['电压', `${system.voltage_v} V`])
  for (const [key, name] of [['gm6020', '偏航电机'], ['dm4310', '俯仰电机']] as const) {
    const m = d.neck?.[key]
    if (m) list.push([name, `${m.state} · ${m.fault} · ${m.temperature_c}℃`])
  }
  if (d.h7?.firmware) list.push(['H7 固件', d.h7.firmware])
  return list
})

const events = computed(() => (remote.state.value?.devices?.events ?? []).slice(-8).reverse())
</script>

<template>
  <div class="flex flex-col gap-4">
    <p v-if="remote.state.value?.fault" class="rounded-lg bg-destructive/10 p-3 text-sm text-destructive">
      {{ remote.state.value.fault }}
    </p>
    <dl class="divide-y rounded-lg border text-sm">
      <div v-for="[k, v] in rows" :key="k" class="flex justify-between gap-4 px-3 py-2.5">
        <dt class="text-muted-foreground">{{ k }}</dt>
        <dd class="text-right tabular-nums">{{ v }}</dd>
      </div>
    </dl>
    <div v-if="events.length">
      <h3 class="mb-2 text-sm font-medium">最近事件</h3>
      <ul class="space-y-1 font-mono text-xs text-muted-foreground">
        <li v-for="(e, i) in events" :key="i" class="truncate">{{ e.operation }} {{ e.value ? JSON.stringify(e.value) : '' }}</li>
      </ul>
    </div>
    <details class="text-xs">
      <summary class="cursor-pointer text-muted-foreground">原始状态</summary>
      <pre class="mt-2 max-h-80 overflow-auto rounded-lg bg-muted p-2">{{ JSON.stringify(remote.state.value, null, 2) }}</pre>
    </details>
  </div>
</template>
