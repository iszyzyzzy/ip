<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Gamepad2, RefreshCw, Save } from 'lucide-vue-next'
import type { GamepadStatus } from '@/lib/types'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()
const padStatus = ref<GamepadStatus | null>(null)
const modifier = ref<'RB' | 'L3' | 'R3'>('RB')
const button = ref<'A' | 'B' | 'X' | 'Y'>('A')
const action = ref('')
const busy = ref(false)
const msg = ref('')

const chord = computed(() => `${modifier.value}+${button.value}`)
const actionNames = computed(() => wan.actions.value.filter(r => !r.deleted).map(r => r.action.name))
const currentBound = computed(() => padStatus.value?.bindings?.[chord.value] ?? '')

watch(currentBound, val => {
  action.value = val
}, { immediate: true })

async function load() {
  if (!wan.online.value) return
  try {
    const [res] = await Promise.all([
      wan.rpc<GamepadStatus>('get_gamepad'),
      wan.loadActions(),
    ])
    padStatus.value = res
    action.value = res.bindings?.[chord.value] ?? ''
  } catch (err) {
    msg.value = String((err as Error).message || err)
  }
}

onMounted(load)
watch(() => wan.online.value, on => { if (on) load() })

function selectExisting(key: string) {
  const [m, b] = key.split('+')
  if (m && b) {
    modifier.value = m as any
    button.value = b as any
  }
}

async function save() {
  if (!wan.online.value || busy.value) return
  busy.value = true
  msg.value = ''
  try {
    const res = await wan.rpc<{ bindings: Record<string, string> }>('set_gamepad', {
      chord: chord.value,
      action: action.value,
    })
    if (padStatus.value) {
      padStatus.value.bindings = res.bindings
    } else {
      await load()
    }
    msg.value = `组合键 ${chord.value} 已保存`
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="pad-grid dv-scroll">
    <section class="dv-card pad-card">
      <div class="dv-section-title">
        <span><Gamepad2 :size="14" /> 树莓派本机蓝牙手柄绑定</span>
        <button class="dv-btn dv-btn-sm" :disabled="!wan.online.value" @click="load">
          <RefreshCw :size="12" /> 刷新
        </button>
      </div>

      <div class="status-bar">
        <span>设备：<strong>{{ padStatus?.device || '车端未检测到手柄' }}</strong></span>
        <span class="dv-pill" :class="padStatus?.controlling ? 'ok' : ''">
          {{ padStatus?.controlling ? '手柄接管中' : '未接管' }}
        </span>
      </div>
      <div v-if="padStatus?.error" class="err-line">{{ padStatus.error }}</div>

      <div v-if="msg" class="msg-banner">{{ msg }}</div>

      <div class="form-row">
        <label>
          按住修饰键
          <select v-model="modifier" class="dv-select">
            <option v-for="m in ['RB', 'L3', 'R3']" :key="m" :value="m">{{ m }}</option>
          </select>
        </label>
        <label>
          再按功能键
          <select v-model="button" class="dv-select">
            <option v-for="b in ['A', 'B', 'X', 'Y']" :key="b" :value="b">{{ b }}</option>
          </select>
        </label>
        <label class="grow">
          绑定动作 ({{ chord }})
          <select v-model="action" class="dv-select">
            <option value="">清除绑定</option>
            <option v-if="currentBound && !actionNames.includes(currentBound)" :value="currentBound">
              {{ currentBound }}
            </option>
            <option v-for="a in actionNames" :key="a" :value="a">{{ a }}</option>
          </select>
        </label>
      </div>

      <div>
        <button class="dv-btn dv-btn-ok" :disabled="!wan.online.value || busy || currentBound === action" @click="save">
          <Save :size="14" /> 保存组合键绑定
        </button>
      </div>
    </section>

    <section class="dv-card pad-card">
      <div class="dv-section-title">
        <span>组合键映射</span>
      </div>
      <div v-if="!padStatus || !Object.keys(padStatus.bindings || {}).length" class="empty">
        暂无已配置的车端手柄绑定
      </div>
      <div v-else class="bindings-list">
        <button
          v-for="(act, key) in padStatus.bindings"
          :key="key"
          class="dv-btn binding-row"
          :class="{ active: key === chord }"
          @click="selectExisting(key)"
        >
          <span class="mono">{{ key }}</span>
          <span>{{ act }}</span>
        </button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.pad-grid {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
  padding-right: 4px;
}

.pad-card {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.status-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 10px;
  border: 1px solid rgba(0, 216, 255, 0.22);
  background: rgba(4, 16, 40, 0.72);
  border-radius: 3px;
  font-size: 13px;
}

.tip {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.5;
}

.form-row {
  display: flex;
  gap: 10px;
  align-items: end;
}

.form-row label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-dim);
  min-width: 110px;
}

.form-row label.grow {
  flex: 1;
}

.bindings-list {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
}

.binding-row {
  justify-content: space-between;
}

.msg-banner {
  padding: 6px 10px;
  border: 1px solid rgba(0, 216, 255, 0.45);
  background: rgba(8, 42, 92, 0.7);
  border-radius: 3px;
  font-size: 12px;
  color: var(--cyan-bright);
}

.err-line {
  padding: 6px 10px;
  border: 1px solid rgba(255, 59, 83, 0.7);
  background: rgba(130, 12, 28, 0.45);
  color: #ffb8c0;
  font-size: 12px;
  border-radius: 3px;
}

.empty {
  color: var(--text-dim);
  font-size: 12px;
}
</style>
