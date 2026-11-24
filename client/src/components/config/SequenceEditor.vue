<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ArrowDown, ArrowUp, Play, Plus, RefreshCw, Sliders, Square, Trash2, X } from 'lucide-vue-next'
import CurveEditor from '@/components/config/CurveEditor.vue'
import { CURVE_TEMPLATES, cloneKeyframes, validateCustomCurve } from '@/lib/curve-math'
import type { ActionRecord, CustomCurve, MotionCurveType } from '@/lib/types'
import { useWanClient } from '@/lib/wan-client'

interface StepRow {
  id: number
  kind: 'preset' | 'sound' | 'eye'
  preset: string
  move: number | ''
  hold: number
  sound: string
  eye: string
  overrideCurve: boolean
  motionCurve: MotionCurveType
  customCurve?: CustomCurve
  expandCurve: boolean
}

const wan = useWanClient()
const busy = ref(false)
const msg = ref('')
const editing = ref<ActionRecord | null>(null)
const name = ref('')
const seqCurve = ref<MotionCurveType>('S_CURVE')
const seqCustom = ref<CustomCurve>({ keyframes: cloneKeyframes(CURVE_TEMPLATES[1]!.keyframes) })
const rows = ref<StepRow[]>([])
let rowId = 0

const activeRecords = computed(() => wan.actions.value.filter(r => !r.deleted))
const presets = computed(() => activeRecords.value.filter(r => r.action.preset))
const sequences = computed(() => activeRecords.value.filter(r => r.action.sequence))
const runningName = computed(() => wan.state.value?.action || '')

onMounted(() => {
  if (wan.online.value) {
    wan.loadActions()
    wan.loadSounds()
  }
})
watch(() => wan.online.value, on => {
  if (on) {
    wan.loadActions()
    wan.loadSounds()
  }
})

function newSequence() {
  editing.value = null
  name.value = ''
  seqCurve.value = 'S_CURVE'
  seqCustom.value = { keyframes: cloneKeyframes(CURVE_TEMPLATES[1]!.keyframes) }
  rows.value = presets.value[0]
    ? [{
        id: ++rowId,
        kind: 'preset',
        preset: presets.value[0].action.name,
        move: '',
        hold: 0.5,
        sound: wan.sounds.value[0] ?? '',
        eye: 'HAPPY',
        overrideCurve: false,
        motionCurve: 'S_CURVE',
        expandCurve: false,
      }]
    : []
  msg.value = ''
}

function editSequence(record: ActionRecord) {
  editing.value = record
  name.value = record.action.name
  const seq = record.action.sequence
  seqCurve.value = seq?.motion_curve || 'SERVO_NATIVE'
  seqCustom.value = seq?.custom_curve?.keyframes?.length
    ? { keyframes: cloneKeyframes(seq.custom_curve.keyframes) }
    : { keyframes: cloneKeyframes(CURVE_TEMPLATES[1]!.keyframes) }

  rows.value = (seq?.steps ?? []).map(step => {
    if (step.event?.sound != null) {
      return {
        id: ++rowId,
        kind: 'sound',
        preset: presets.value[0]?.action.name ?? '',
        move: '',
        hold: (step.hold_ms ?? 0) / 1000,
        sound: step.event.sound,
        eye: 'NORMAL',
        overrideCurve: false,
        motionCurve: 'SERVO_NATIVE',
        expandCurve: false,
      }
    }
    if (step.event?.eye_expression != null) {
      return {
        id: ++rowId,
        kind: 'eye',
        preset: presets.value[0]?.action.name ?? '',
        move: '',
        hold: (step.hold_ms ?? 0) / 1000,
        sound: wan.sounds.value[0] ?? '',
        eye: step.event.eye_expression,
        overrideCurve: false,
        motionCurve: 'SERVO_NATIVE',
        expandCurve: false,
      }
    }
    return {
      id: ++rowId,
      kind: 'preset',
      preset: step.preset ?? '',
      move: step.move_ms == null ? '' : step.move_ms / 1000,
      hold: (step.hold_ms ?? 0) / 1000,
      sound: wan.sounds.value[0] ?? '',
      eye: 'HAPPY',
      overrideCurve: step.motion_curve != null,
      motionCurve: step.motion_curve ?? seqCurve.value,
      customCurve: step.custom_curve?.keyframes?.length
        ? { keyframes: cloneKeyframes(step.custom_curve.keyframes) }
        : undefined,
      expandCurve: false,
    }
  })
  msg.value = ''
}

function addStep(kind: 'preset' | 'sound' | 'eye') {
  rows.value.push({
    id: ++rowId,
    kind,
    preset: presets.value[0]?.action.name ?? '',
    move: '',
    hold: kind === 'preset' ? 0.5 : 0,
    sound: wan.sounds.value[0] ?? '',
    eye: 'HAPPY',
    overrideCurve: false,
    motionCurve: seqCurve.value,
    customCurve: undefined,
    expandCurve: false,
  })
}

function reorder(index: number, offset: number) {
  const next = index + offset
  if (next < 0 || next >= rows.value.length) return
  const [item] = rows.value.splice(index, 1)
  rows.value.splice(next, 0, item!)
}

function stepDurationMs(row: StepRow): number {
  if (row.move !== '' && Number(row.move) > 0) return Math.round(Number(row.move) * 1000)
  const found = presets.value.find(p => p.action.name === row.preset)
  return found?.action.preset?.move_ms ?? 1000
}

function toggleStepOverride(row: StepRow) {
  row.expandCurve = !row.expandCurve
  if (row.expandCurve && !row.overrideCurve) {
    row.motionCurve = seqCurve.value
    row.customCurve = seqCurve.value === 'CUSTOM'
      ? { keyframes: cloneKeyframes(seqCustom.value.keyframes) }
      : undefined
  }
}

async function save() {
  if (busy.value) return
  const key = name.value.trim()
  if (!key) {
    msg.value = '请输入序列名称'
    return
  }
  if (!rows.value.length) {
    msg.value = '请至少添加一个动作步骤'
    return
  }
  if (seqCurve.value === 'CUSTOM') {
    const err = validateCustomCurve(seqCustom.value.keyframes)
    if (err) {
      msg.value = `序列默认曲线无效：${err}`
      return
    }
  }
  for (let i = 0; i < rows.value.length; i++) {
    const r = rows.value[i]!
    if (r.kind === 'preset' && r.overrideCurve && r.motionCurve === 'CUSTOM') {
      const err = validateCustomCurve(r.customCurve?.keyframes ?? [])
      if (err) {
        msg.value = `第 ${i + 1} 步自定义曲线无效：${err}`
        return
      }
    }
    if (r.kind === 'sound' && !r.sound.trim()) {
      msg.value = `第 ${i + 1} 步缺少语音文件名`
      return
    }
  }

  const existing = wan.actions.value.find(r => r.action.name === key)
  if (existing && !existing.deleted && !editing.value) {
    msg.value = '同名动作已存在，请从左侧列表选择编辑'
    return
  }
  const expected_revision = editing.value?.revision ?? existing?.revision ?? '0'

  const steps = rows.value.map(r => {
    const hold_ms = Math.max(0, Math.round(Number(r.hold || 0) * 1000))
    if (r.kind === 'sound') {
      return { event: { sound: r.sound.trim() }, hold_ms }
    }
    if (r.kind === 'eye') {
      return { event: { eye_expression: r.eye }, hold_ms }
    }
    const step: Record<string, unknown> = {
      preset: r.preset,
      hold_ms,
    }
    if (r.move !== '' && Number(r.move) > 0) {
      step.move_ms = Math.round(Number(r.move) * 1000)
    }
    if (r.overrideCurve) {
      step.motion_curve = r.motionCurve
      if (r.motionCurve === 'CUSTOM' && r.customCurve) {
        step.custom_curve = r.customCurve
      }
    }
    return step
  })

  const sequence: Record<string, unknown> = {
    motion_curve: seqCurve.value,
    steps,
  }
  if (seqCurve.value === 'CUSTOM') {
    sequence.custom_curve = seqCustom.value
  }

  busy.value = true
  msg.value = ''
  try {
    const saved = await wan.rpc<ActionRecord>('put_action', {
      expected_revision,
      action: { name: key, sequence },
    })
    await wan.loadActions()
    editSequence(saved)
    msg.value = `序列「${key}」保存成功`
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    busy.value = false
  }
}

async function remove() {
  if (!editing.value || busy.value) return
  if (!confirm(`确定删除序列「${editing.value.action.name}」？`)) return
  busy.value = true
  try {
    await wan.rpc('delete_action', {
      name: editing.value.action.name,
      expected_revision: editing.value.revision,
    })
    await wan.loadActions()
    newSequence()
    msg.value = '序列已删除'
  } catch (err) {
    msg.value = String((err as Error).message || err)
  } finally {
    busy.value = false
  }
}

function runSeq(seqName: string) {
  wan.runAction(seqName).catch(err => {
    msg.value = String((err as Error).message || err)
  })
}
</script>

<template>
  <div class="seq-layout">
    <!-- 左侧：序列列表 -->
    <div class="seq-list-pane dv-card">
      <div class="dv-section-title">
        <span>动作序列库</span>
        <div class="row-gap">
          <button class="dv-btn dv-btn-sm" :disabled="!wan.online.value" @click="wan.loadActions(); wan.loadSounds()">
            <RefreshCw :size="12" />
          </button>
          <button class="dv-btn dv-btn-sm dv-btn-ok" @click="newSequence">
            <Plus :size="12" /> 新建
          </button>
        </div>
      </div>

      <div class="seq-items dv-scroll">
        <div v-if="!sequences.length" class="empty">暂无动作序列</div>
        <div
          v-for="item in sequences"
          :key="item.action.name"
          class="seq-item"
          :class="{ active: editing?.action.name === item.action.name }"
          @click="editSequence(item)"
        >
          <div class="seq-meta">
            <strong>{{ item.action.name }}</strong>
            <span class="mono">
              {{ item.action.sequence?.steps.length }} 步 · {{ item.action.sequence?.motion_curve || 'SERVO_NATIVE' }}
            </span>
          </div>
          <button
            class="dv-btn dv-btn-sm"
            :disabled="!wan.controlling.value || !!runningName"
            title="运行测试"
            @click.stop="runSeq(item.action.name)"
          >
            <Play :size="12" />
          </button>
        </div>
      </div>
    </div>

    <!-- 右侧：序列与 AE 曲线编辑器 -->
    <div class="seq-editor-pane dv-scroll">
      <div class="dv-section-title">
        <span>{{ editing ? `编辑序列：${editing.action.name}` : '新建动作序列与补插曲线' }}</span>
        <div class="row-gap">
          <button
            v-if="runningName"
            class="dv-btn dv-btn-sm dv-btn-danger"
            :disabled="!wan.controlling.value"
            @click="wan.cancelAction()"
          >
            <Square :size="12" /> 取消运行 ({{ runningName }})
          </button>
          <button
            v-if="editing"
            class="dv-btn dv-btn-sm dv-btn-danger"
            :disabled="!wan.online.value || busy"
            @click="remove"
          >
            <Trash2 :size="12" /> 删除
          </button>
          <button
            class="dv-btn dv-btn-sm dv-btn-ok"
            :disabled="!wan.online.value || busy"
            @click="save"
          >
            保存序列
          </button>
        </div>
      </div>

      <div v-if="msg" class="msg-banner">{{ msg }}</div>

      <div class="name-row">
        <label>
          序列名称
          <input v-model="name" class="dv-input" maxlength="64" :readonly="!!editing" />
        </label>
      </div>

      <!-- 序列级默认补插曲线（AE 关键帧编辑器） -->
      <div class="block-label">默认补插曲线</div>
      <CurveEditor
        :model-curve="seqCurve"
        :model-custom="seqCustom"
        :duration-ms="1500"
        @update:model-curve="seqCurve = $event"
        @update:model-custom="$event && (seqCustom = $event)"
      />

      <!-- 步骤列表 -->
      <div class="steps-header">
        <span class="block-label">步骤时间线 · {{ rows.length }} 步</span>
        <div class="row-gap">
          <button class="dv-btn dv-btn-sm" :disabled="!presets.length" @click="addStep('preset')">
            <Plus :size="12" /> 预设步
          </button>
          <button class="dv-btn dv-btn-sm" @click="addStep('sound')">
            <Plus :size="12" /> 语音事件
          </button>
          <button class="dv-btn dv-btn-sm" @click="addStep('eye')">
            <Plus :size="12" /> 表情事件
          </button>
        </div>
      </div>

      <div class="steps-list">
        <div v-for="(row, idx) in rows" :key="row.id" class="step-card dv-card">
          <div class="step-main">
            <span class="step-idx mono">#{{ idx + 1 }}</span>

            <template v-if="row.kind === 'preset'">
              <span class="dv-pill">预设</span>
              <select v-model="row.preset" class="dv-select step-select">
                <option v-for="p in presets" :key="p.action.name" :value="p.action.name">
                  {{ p.action.name }}
                </option>
              </select>
              <label class="inline-num">
                运动(s)
                <input v-model.number="row.move" type="number" min="0.001" max="9.999" step="0.1" class="dv-input" placeholder="默认" />
              </label>
              <label class="inline-num">
                停留(s)
                <input v-model.number="row.hold" type="number" min="0" max="600" step="0.1" class="dv-input" />
              </label>
              <button
                class="dv-btn dv-btn-sm"
                :class="{ active: row.overrideCurve || row.expandCurve }"
                title="为该步单独配置补插曲线"
                @click="toggleStepOverride(row)"
              >
                <Sliders :size="12" />
                {{ row.overrideCurve ? row.motionCurve : '曲线' }}
              </button>
            </template>

            <template v-else-if="row.kind === 'sound'">
              <span class="dv-pill ok">语音</span>
              <select v-if="wan.sounds.value.length" v-model="row.sound" class="dv-select step-select">
                <option v-for="s in wan.sounds.value" :key="s" :value="s">{{ s }}</option>
              </select>
              <input v-else v-model="row.sound" class="dv-input step-select" placeholder="音频文件名" />
              <label class="inline-num">
                等待(s)
                <input v-model.number="row.hold" type="number" min="0" max="600" step="0.1" class="dv-input" />
              </label>
            </template>

            <template v-else>
              <span class="dv-pill warn">表情</span>
              <select v-model="row.eye" class="dv-select step-select">
                <option value="NORMAL">普通</option>
                <option value="HAPPY">笑眼</option>
                <option value="EXCITED">兴奋</option>
              </select>
              <label class="inline-num">
                等待(s)
                <input v-model.number="row.hold" type="number" min="0" max="600" step="0.1" class="dv-input" />
              </label>
            </template>

            <div class="step-ops">
              <button class="dv-btn dv-btn-sm" :disabled="idx === 0" @click="reorder(idx, -1)">
                <ArrowUp :size="12" />
              </button>
              <button class="dv-btn dv-btn-sm" :disabled="idx === rows.length - 1" @click="reorder(idx, 1)">
                <ArrowDown :size="12" />
              </button>
              <button class="dv-btn dv-btn-sm dv-btn-danger" @click="rows.splice(idx, 1)">
                <X :size="12" />
              </button>
            </div>
          </div>

          <div v-if="row.kind === 'preset' && row.expandCurve" class="step-curve-drawer">
            <CurveEditor
              :allow-inherit="true"
              :is-inherited="!row.overrideCurve"
              :model-curve="row.motionCurve"
              :model-custom="row.customCurve"
              :duration-ms="stepDurationMs(row)"
              @clear-override="row.overrideCurve = false"
              @update:model-curve="row.overrideCurve = true; row.motionCurve = $event"
              @update:model-custom="row.overrideCurve = true; row.customCurve = $event"
            />
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.seq-layout {
  display: grid;
  grid-template-columns: 260px 1fr;
  gap: 12px;
  height: 100%;
  min-height: 0;
}

.seq-list-pane {
  display: flex;
  flex-direction: column;
  padding: 10px;
  min-height: 0;
}

.seq-items {
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex: 1;
  min-height: 0;
}

.seq-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 8px 10px;
  border: 1px solid rgba(0, 216, 255, 0.22);
  border-radius: 4px;
  background: rgba(4, 18, 44, 0.72);
  cursor: pointer;
}

.seq-item.active,
.seq-item:hover {
  border-color: var(--cyan-bright);
  background: rgba(12, 52, 112, 0.75);
}

.seq-meta {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}

.seq-meta strong {
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.seq-meta span {
  font-size: 11px;
  color: var(--text-dim);
}

.seq-editor-pane {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-height: 0;
  padding-right: 4px;
}

.row-gap {
  display: flex;
  align-items: center;
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

.name-row label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: var(--text-dim);
}

.block-label {
  font-size: 12px;
  font-weight: 700;
  color: var(--cyan-bright);
}

.steps-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-top: 4px;
}

.hint-warn {
  padding: 8px 10px;
  border: 1px solid rgba(255, 176, 32, 0.55);
  background: rgba(80, 48, 0, 0.45);
  color: #ffd37d;
  font-size: 12px;
  border-radius: 3px;
}

.steps-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.step-card {
  padding: 8px 10px;
}

.step-main {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}

.step-idx {
  width: 28px;
  font-size: 12px;
  color: var(--cyan-bright);
}

.step-select {
  width: 170px;
}

.inline-num {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text-dim);
}

.inline-num .dv-input {
  width: 78px;
  padding: 4px 6px;
}

.step-ops {
  margin-left: auto;
  display: flex;
  gap: 4px;
}

.step-curve-drawer {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px dashed rgba(0, 216, 255, 0.28);
}

.empty {
  padding: 20px;
  text-align: center;
  font-size: 12px;
  color: var(--text-dim);
}
</style>
