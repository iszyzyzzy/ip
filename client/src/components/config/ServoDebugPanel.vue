<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Sliders } from 'lucide-vue-next'
import { useWanClient } from '@/lib/wan-client'

const props = defineProps<{ arm?: 'left_arm' | 'right_arm' }>()
const wan = useWanClient()
const selectedSide = ref<'left_arm' | 'right_arm'>('left_arm')
const side = computed(() => props.arm ?? selectedSide.value)
const drafts = reactive<Record<string, number>>({})
const sending = ref(false)
const msg = ref('')

// 关节限位标定弹窗状态
const calOpen = ref<number | null>(null)
const calDraft = reactive({ low: 1000, zero: 1500, high: 2000 })
const calRevision = ref('')
const calSaving = ref(false)

const armConfig = computed(() => wan.config.value?.[side.value])

const rows = computed(() => (armConfig.value?.joints ?? []).map((joint: any, index: number) => {
  const feedback = wan.state.value?.devices?.[side.value]?.[String(index)]
  const fresh = wan.online.value && !!feedback && !feedback.error && Number.isFinite(feedback.pwm)
  return {
    id: joint.id,
    index,
    key: `${side.value}-${index}`,
    joint,
    feedback,
    fresh,
  }
}))

watch(rows, values => {
  for (const row of values) {
    if (row.fresh && drafts[row.key] == null) {
      drafts[row.key] = row.feedback.pwm
    }
  }
}, { immediate: true })

async function move(index: number, value: number, restoreHold = false) {
  if (!wan.controlling.value || sending.value || !armConfig.value?.enabled) return
  if (!Number.isInteger(value) || value < 500 || value > 2500) {
    msg.value = '舵机位置须为 500～2500 的整数'
    return
  }
  sending.value = true
  msg.value = ''
  try {
    await wan.setTargets({
      servo: {
        right_arm: side.value === 'right_arm',
        joint_index: index,
        position_pwm: value,
        restore_hold: restoreHold,
      },
    })
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    sending.value = false
  }
}

function nudge(index: number, delta: number) {
  const row = rows.value[index]
  if (!row) return
  const base = drafts[row.key] ?? row.feedback?.pwm ?? 1500
  const next = Math.max(500, Math.min(2500, Math.round(base + delta)))
  drafts[row.key] = next
  move(index, next)
}

function openCalibration(index: number) {
  const j = armConfig.value?.joints?.[index]
  if (!j) return
  const ends = [j.min_rad, j.max_rad].map((a: number) => Math.round(j.center_pwm + a * j.direction * j.pwm_per_rad))
  calDraft.low = Math.min(...ends)
  calDraft.zero = j.center_pwm
  calDraft.high = Math.max(...ends)
  calRevision.value = wan.configRevision.value
  calOpen.value = index
}

const calError = computed(() => {
  if (calOpen.value == null) return ''
  if (wan.configRevision.value !== calRevision.value) return '车端配置已变化，请关闭后重新打开标定'
  const j = armConfig.value?.joints?.[calOpen.value]
  const { low, zero, high } = calDraft
  if (![low, zero, high].every(v => Number.isInteger(v) && v >= 500 && v <= 2500)) return '读数须为 500～2500 的整数'
  if (low >= high || zero < low || zero > high) return '最小读数应小于最大读数，且零位须落在区间内'
  if (j && Math.max(high - zero, zero - low) > Math.PI * j.pwm_per_rad) return '可动范围单侧超过 180°'
  return ''
})

async function saveCalibration() {
  if (calOpen.value == null || calError.value || calSaving.value || !wan.config.value) return
  calSaving.value = true
  msg.value = ''
  try {
    const cfg = JSON.parse(JSON.stringify(wan.config.value))
    const j = cfg[side.value].joints[calOpen.value]
    const ends = [calDraft.low, calDraft.high].map(p => (p - calDraft.zero) / (j.direction * j.pwm_per_rad))
    Object.assign(j, {
      center_pwm: calDraft.zero,
      min_rad: Math.min(...ends),
      max_rad: Math.max(...ends),
    })
    await wan.rpc('update_config', { expected_revision: calRevision.value, config: cfg })
    await wan.loadConfig()
    calOpen.value = null
    msg.value = '关节限位与零点标定已保存'
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    calSaving.value = false
  }
}
</script>

<template>
  <div class="servo-root dv-scroll">
    <div v-if="!arm" class="top-switch">
      <div class="side-tabs">
        <button class="dv-btn" :class="{ active: side === 'left_arm' }" @click="selectedSide = 'left_arm'; calOpen = null">
          左臂 · U2
        </button>
        <button class="dv-btn" :class="{ active: side === 'right_arm' }" @click="selectedSide = 'right_arm'; calOpen = null">
          右臂 · U3
        </button>
      </div>
    </div>

    <div v-if="msg" class="msg-banner">{{ msg }}</div>
    <div v-if="!armConfig?.enabled" class="dv-card empty-tip">未启用</div>

    <div v-else class="joints-grid">
      <div v-for="row in rows" :key="row.key" class="dv-card joint-card">
        <div class="dv-section-title">
          <span>关节 {{ row.index + 1 }} (ID {{ row.id }})</span>
          <span class="mono" :class="row.fresh ? 'ok-txt' : 'dim-txt'">
            {{ row.fresh ? `实测 ${row.feedback.pwm}` : (row.feedback?.error || '无反馈/等待') }}
          </span>
        </div>

        <input
          type="range"
          min="500"
          max="2500"
          step="1"
          class="pwm-slider"
          :value="drafts[row.key] ?? 1500"
          :disabled="!wan.controlling.value || !row.fresh || sending"
          @input="drafts[row.key] = Number(($event.target as HTMLInputElement).value)"
          @change="move(row.index, drafts[row.key]!)"
        />

        <div class="ctrl-row">
          <button class="dv-btn dv-btn-sm" :disabled="!wan.controlling.value || !row.fresh || sending" @click="nudge(row.index, -10)">-10</button>
          <input
            v-model.number="drafts[row.key]"
            type="number"
            min="500"
            max="2500"
            class="dv-input mono pwm-num"
          />
          <button class="dv-btn dv-btn-sm" :disabled="!wan.controlling.value || !row.fresh || sending" @click="nudge(row.index, 10)">+10</button>
          <button class="dv-btn dv-btn-sm dv-btn-ok" :disabled="!wan.controlling.value || !row.fresh || sending" @click="move(row.index, drafts[row.key]!)">
            移动
          </button>
        </div>

        <div class="sub-ops">
          <button
            class="dv-btn dv-btn-sm"
            :disabled="!wan.controlling.value || !row.fresh || sending"
            @click="move(row.index, row.feedback.pwm, true)"
          >
            恢复保持
          </button>
          <button v-if="!arm" class="dv-btn dv-btn-sm" @click="calOpen === row.index ? (calOpen = null) : openCalibration(row.index)">
            <Sliders :size="12" /> 标定范围
          </button>
        </div>

        <div v-if="calOpen === row.index" class="cal-box">
          <div class="cal-row" v-for="f in [
            { k: 'low', label: '最小读数' },
            { k: 'zero', label: '零位读数' },
            { k: 'high', label: '最大读数' },
          ] as const" :key="f.k">
            <span class="cal-label">{{ f.label }}</span>
            <input v-model.number="calDraft[f.k]" type="number" min="500" max="2500" class="dv-input mono" />
            <button
              class="dv-btn dv-btn-sm"
              :disabled="!row.fresh"
              @click="row.fresh && (calDraft[f.k] = row.feedback.pwm)"
            >
              取实际值
            </button>
          </div>
          <div v-if="calError" class="cal-err">{{ calError }}</div>
          <div class="cal-actions">
            <button v-if="!arm" class="dv-btn dv-btn-sm" @click="calOpen = null">取消</button>
            <button
              class="dv-btn dv-btn-sm dv-btn-ok"
              :disabled="!!calError || !wan.controlling.value || calSaving"
              @click="saveCalibration"
            >
              保存标定
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.servo-root {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  padding-right: 4px;
}

.top-switch {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.side-tabs {
  display: flex;
  gap: 8px;
}

.hint {
  font-size: 12px;
  color: var(--text-dim);
}

.joints-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
}

.joint-card {
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.ok-txt { color: #7effcf; }
.dim-txt { color: var(--text-dim); }

.pwm-slider {
  width: 100%;
  accent-color: var(--cyan);
}

.ctrl-row {
  display: flex;
  gap: 6px;
  align-items: center;
}

.pwm-num {
  flex: 1;
  min-width: 0;
  text-align: center;
}

.sub-ops {
  display: flex;
  justify-content: space-between;
  gap: 6px;
}

.cal-box {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding-top: 8px;
  border-top: 1px dashed rgba(0, 216, 255, 0.3);
}

.cal-row {
  display: grid;
  grid-template-columns: 70px 1fr 78px;
  gap: 6px;
  align-items: center;
}

.cal-label {
  font-size: 12px;
  color: var(--text-dim);
}

.cal-err {
  font-size: 12px;
  color: #ff8e9b;
}

.cal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 6px;
}

.msg-banner {
  padding: 6px 10px;
  border: 1px solid rgba(0, 216, 255, 0.45);
  background: rgba(8, 42, 92, 0.7);
  border-radius: 3px;
  font-size: 12px;
  color: var(--cyan-bright);
}

.empty-tip {
  padding: 20px;
  text-align: center;
  color: var(--text-dim);
}
</style>
