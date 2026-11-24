<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { Camera, Play, Plus, RefreshCw, Trash2 } from 'lucide-vue-next'
import type { ActionRecord } from '@/lib/types'
import { useWanClient } from '@/lib/wan-client'

import JoystickPad from '@/components/dashboard/sub/JoystickPad.vue'
import ServoDebugPanel from './ServoDebugPanel.vue'
import { useInputController } from '@/lib/input-controller'

const input = useInputController()
const wan = useWanClient()
const busy = ref(false)
const msg = ref('')
const editing = ref<ActionRecord | null>(null)
const name = ref('')
const moveSeconds = ref(1.0)
const expression = ref('')
const include = ref({ left_arm: true, right_arm: true, neck: false })

const parts = [
  { id: 'head', label: '头部' },
  { id: 'left_arm', label: '左臂' },
  { id: 'right_arm', label: '右臂' },
] as const
const activePart = ref<typeof parts[number]['id']>('head')
function degrees(radians: unknown) {
  return typeof radians === 'number' && Number.isFinite(radians)
    ? (radians * 180 / Math.PI).toFixed(1) + '°' : '—'
}
const headAngles = computed(() => {
  const neck = wan.state.value?.devices?.neck
  return [
    { label: '偏航', target: input.neckPose.yaw, actual: neck?.gm6020?.joint_position_rad },
    { label: '俯仰', target: input.neckPose.pitch, actual: neck?.dm4310?.joint_position_rad },
  ]
})
const presets = computed(() => wan.actions.value.filter(r => !r.deleted && r.action.preset))
const runningName = computed(() => wan.state.value?.action || '')

onMounted(() => {
  if (wan.online.value) wan.loadActions()
})
watch(() => wan.online.value, on => {
  if (on) wan.loadActions()
})

function resetNew() {
  editing.value = null
  name.value = ''
  moveSeconds.value = 1.0
  expression.value = ''
  include.value = {
    left_arm: !!wan.config.value?.left_arm?.enabled,
    right_arm: !!wan.config.value?.right_arm?.enabled,
    neck: false,
  }
  msg.value = ''
}

function selectPreset(record: ActionRecord) {
  editing.value = record
  name.value = record.action.name
  const p = record.action.preset
  moveSeconds.value = (p?.move_ms ?? 1000) / 1000
  expression.value = p?.targets.eye_expression ?? ''
  include.value = {
    left_arm: !!p?.targets.left_arm,
    right_arm: !!p?.targets.right_arm,
    neck: !!p?.targets.neck,
  }
  msg.value = ''
}

async function save(capturePose: boolean) {
  if (busy.value) return
  const key = name.value.trim()
  if (!key) {
    msg.value = '请输入预设名称'
    return
  }
  const existing = wan.actions.value.find(r => r.action.name === key)
  if (existing && !existing.deleted && !editing.value) {
    msg.value = '同名动作已存在，请从左侧列表选择编辑'
    return
  }
  const expected_revision = editing.value?.revision ?? existing?.revision ?? '0'
  const move_ms = Math.max(1, Math.min(9999, Math.round(Number(moveSeconds.value) * 1000)))
  const eye_expression = expression.value || undefined

  busy.value = true
  msg.value = ''
  try {
    let saved: ActionRecord
    if (capturePose) {
      saved = await wan.rpc('capture_preset', {
        name: key,
        expected_revision,
        move_ms,
        eye_expression,
        left_arm: include.value.left_arm && !!wan.config.value?.left_arm?.enabled,
        right_arm: include.value.right_arm && !!wan.config.value?.right_arm?.enabled,
        neck: include.value.neck && !!wan.config.value?.neck?.enabled,
      })
    } else {
      saved = await wan.rpc('put_action', {
        expected_revision,
        action: {
          name: key,
          preset: {
            move_ms,
            targets: {
              ...editing.value?.action.preset?.targets,
              eye_expression,
            },
          },
        },
      })
    }
    await wan.loadActions()
    selectPreset(saved)
    msg.value = `预设「${key}」已保存`
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    busy.value = false
  }
}

async function remove() {
  if (!editing.value || busy.value) return
  if (!confirm(`删除预设「${editing.value.action.name}」？`)) return
  busy.value = true
  try {
    await wan.rpc('delete_action', {
      name: editing.value.action.name,
      expected_revision: editing.value.revision,
    })
    await wan.loadActions()
    resetNew()
    msg.value = '预设已删除'
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="preset-layout">
    <div class="preset-list dv-card">
      <div class="dv-section-title">
        <span>动作预设库</span>
        <div class="row-gap">
          <button class="dv-btn dv-btn-sm" :disabled="!wan.online.value" @click="wan.loadActions()">
            <RefreshCw :size="12" />
          </button>
          <button class="dv-btn dv-btn-sm dv-btn-ok" @click="resetNew">
            <Plus :size="12" /> 新建
          </button>
        </div>
      </div>

      <div class="items dv-scroll">
        <div v-if="!presets.length" class="empty">暂无动作预设</div>
        <div
          v-for="item in presets"
          :key="item.action.name"
          class="item"
          :class="{ active: editing?.action.name === item.action.name }"
          @click="selectPreset(item)"
        >
          <div>
            <strong>{{ item.action.name }}</strong>
            <div class="sub mono">{{ ((item.action.preset?.move_ms ?? 1000) / 1000).toFixed(2) }} s</div>
          </div>
          <button
            class="dv-btn dv-btn-sm"
            :disabled="!wan.controlling.value || !!runningName"
            @click.stop="wan.runAction(item.action.name)"
          >
            <Play :size="12" />
          </button>
        </div>
      </div>
    </div>

    <div class="preset-form dv-card dv-scroll">
      <div class="dv-section-title">
        <span>{{ editing ? `编辑预设：${editing.action.name}` : '采集或新建预设姿态' }}</span>
        <button
          v-if="editing"
          class="dv-btn dv-btn-sm dv-btn-danger"
          :disabled="!wan.online.value || busy"
          @click="remove"
        >
          <Trash2 :size="12" /> 删除
        </button>
      </div>

      <div v-if="msg" class="msg-banner">{{ msg }}</div>

      <label class="field">
        预设名称
        <input v-model="name" class="dv-input" maxlength="64" :readonly="!!editing" />
      </label>

      <div class="field">
        <span>包含部位</span>
        <div class="checks">
          <label v-if="wan.config.value?.left_arm?.enabled">
            <input v-model="include.left_arm" type="checkbox" /> 左臂
          </label>
          <label v-if="wan.config.value?.right_arm?.enabled">
            <input v-model="include.right_arm" type="checkbox" /> 右臂
          </label>
          <label v-if="wan.config.value?.neck?.enabled">
            <input v-model="include.neck" type="checkbox" /> 头部
          </label>
        </div>
      </div>

      <div class="two-col">
        <label class="field">
          默认运动时间 (s)
          <input v-model.number="moveSeconds" type="number" min="0.001" max="9.999" step="0.1" class="dv-input" />
        </label>
        <label class="field">
          关联表情
          <select v-model="expression" class="dv-select">
            <option value="">不切换表情</option>
            <option value="NORMAL">普通</option>
            <option value="HAPPY">笑眼</option>
            <option value="EXCITED">兴奋</option>
          </select>
        </label>
      </div>

      <div class="row-gap">
        <button class="dv-btn dv-btn-ok" :disabled="!wan.online.value || busy || !name.trim()" @click="save(true)">
          <Camera :size="14" /> 采集当前位置并保存
        </button>
        <button
          v-if="editing"
          class="dv-btn"
          :disabled="!wan.online.value || busy"
          @click="save(false)"
        >
          仅保存时间与表情
        </button>
      </div>
      <section class="pose-controls dv-card">
        <div class="row-gap" role="group" aria-label="调姿部位">
          <button v-for="part in parts" :key="part.id" class="dv-btn"
            :class="{ active: activePart === part.id }" :aria-pressed="activePart === part.id"
            @click="activePart = part.id">{{ part.label }}</button>
        </div>
        <div v-if="activePart === 'head'" class="pose-head">
          <JoystickPad :size="220" :spring="false" :disabled="input.headDisabled.value" :disabled-label="input.headStatus.value"
            :model-x="input.neckStick.x" :model-y="input.neckStick.y"
            top-label="抬头" @change="input.setNeckStick" />
          <div class="head-readout">
            <table aria-label="头部角度">
              <thead><tr><td></td><th scope="col">目标</th><!-- <th scope="col">实测</th> --></tr></thead>
              <tbody><tr v-for="axis in headAngles" :key="axis.label">
                <th scope="row">{{ axis.label }}</th><td>{{ degrees(axis.target) }}</td><!-- <td>{{ degrees(axis.actual) }}</td> -->
              </tr></tbody>
            </table>
            <button class="dv-btn dv-btn-sm" :disabled="!input.canCenterHead.value" @click="input.centerHead()">头部回中</button>
          </div>
        </div>
        <ServoDebugPanel v-else :arm="activePart" class="pose-arms" />
      </section>
    </div>
  </div>
</template>

<style scoped>
.preset-layout {
  display: grid;
  grid-template-columns: clamp(180px, 18vw, 260px) minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr);
  gap: 12px;
  height: 100%;
  min-height: 0;
}

.pose-controls {
  display: grid;
  gap: 12px;
  padding: 12px;
}
.pose-head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 24px;
  padding: 12px;
}
.head-readout { display: grid; gap: 12px; }
.head-readout table { border-spacing: 10px 5px; font-size: 13px; }
.head-readout th { font-weight: normal; color: var(--text-dim); }
.head-readout td { min-width: 5ch; text-align: right; font-variant-numeric: tabular-nums; }
.pose-arms { height: auto; overflow: visible; }


.preset-list,
.preset-form {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 12px;
  min-height: 0;
}

.items {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
}

.item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 10px;
  border: 1px solid rgba(0, 216, 255, 0.22);
  border-radius: 4px;
  background: rgba(4, 18, 44, 0.72);
  cursor: pointer;
}

.item.active,
.item:hover {
  border-color: var(--cyan-bright);
  background: rgba(12, 52, 112, 0.75);
}

.sub {
  font-size: 11px;
  color: var(--text-dim);
}

.field {
  display: flex;
  flex-direction: column;
  gap: 5px;
  font-size: 12px;
  color: var(--text-dim);
}

.checks {
  display: flex;
  gap: 16px;
  color: var(--text-main);
  font-size: 13px;
}

.two-col {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}

.row-gap {
  display: flex;
  gap: 8px;
}

.msg-banner {
  padding: 6px 10px;
  border: 1px solid rgba(0, 216, 255, 0.45);
  background: rgba(8, 42, 92, 0.7);
  border-radius: 3px;
  font-size: 12px;
  color: var(--cyan-bright);
}

.tip {
  margin: 0;
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.5;
}

.empty {
  padding: 20px;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}
</style>
