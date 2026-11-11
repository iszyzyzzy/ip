<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ArrowDown, ArrowUp, Play, Plus, Square, X } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { type ActionRecord, useRemote } from '@/lib/remote'

const props = defineProps<{ debug: boolean, config: Record<string, any> | null }>()
const remote = useRemote()
const records = ref<ActionRecord[]>([])
const active = computed(() => records.value.filter(record => !record.deleted))
const presets = computed(() => active.value.filter(record => record.action.preset))
const running = computed(() => remote.state.value?.action || '')
const canRun = computed(() => remote.controlling.value && !running.value)
const busy = ref(false)
const kind = ref<'preset' | 'sequence'>('preset')
const name = ref('')
const editing = ref<ActionRecord | null>(null)
const moveSeconds = ref(1)
const expression = ref('')
const include = ref({ left_arm: true, right_arm: true, neck: false })
const expressions = [
  { value: 'NORMAL', label: '普通' },
  { value: 'HAPPY', label: '笑眼' },
  { value: 'EXCITED', label: '(≧▽≦)' },
]
let rowId = 0
const rows = ref<{ id: number, preset: string, move: number | '', hold: number }[]>([])
const ms = (seconds: number) => Math.round(Number(seconds) * 1000)
const selectClass = 'h-10 min-w-0 w-full rounded-md border bg-background px-2 text-sm'

async function load() {
  if (!remote.online.value) return
  try { records.value = (await remote.command<{ actions: ActionRecord[] }>('list_actions')).actions ?? [] }
  catch { /* 网关统一显示错误。 */ }
}
function edit(record: ActionRecord | null, type: 'preset' | 'sequence' = 'preset') {
  editing.value = record
  kind.value = record?.action.sequence ? 'sequence' : record?.action.preset ? 'preset' : type
  name.value = record?.action.name ?? ''
  moveSeconds.value = (record?.action.preset?.move_ms ?? 1000) / 1000
  expression.value = record?.action.preset?.targets.eye_expression ?? ''
  if (record?.action.preset) {
    const targets = record.action.preset.targets
    include.value = { left_arm: !!targets.left_arm, right_arm: !!targets.right_arm, neck: !!targets.neck }
  } else {
    include.value = { left_arm: !!props.config?.left_arm?.enabled, right_arm: !!props.config?.right_arm?.enabled, neck: false }
  }
  rows.value = (record?.action.sequence?.steps ?? []).map(step => ({
    id: ++rowId, preset: step.preset, move: step.move_ms == null ? '' : step.move_ms / 1000, hold: (step.hold_ms ?? 0) / 1000,
  }))
}
function addStep() {
  rows.value.push({ id: ++rowId, preset: presets.value[0]?.action.name ?? '', move: '', hold: 1 })
}
function reorder(index: number, offset: number) {
  const next = index + offset
  if (next < 0 || next >= rows.value.length) return
  const [row] = rows.value.splice(index, 1)
  rows.value.splice(next, 0, row!)
}
async function save(capture = false) {
  if (busy.value || !props.debug || !name.value.trim()) return
  const key = name.value.trim()
  const existing = records.value.find(record => record.action.name === key)
  if (existing && !existing.deleted && !editing.value) {
    remote.message.value = '该名称已存在，请从列表选择编辑。'
    return
  }
  const expected_revision = editing.value?.revision ?? existing?.revision ?? '0'
  const eye_expression = expression.value || undefined
  busy.value = true
  try {
    let saved: ActionRecord
    if (capture) {
      saved = await remote.command('capture_preset', {
        name: key, expected_revision, move_ms: ms(moveSeconds.value), eye_expression,
        left_arm: include.value.left_arm && !!props.config?.left_arm?.enabled,
        right_arm: include.value.right_arm && !!props.config?.right_arm?.enabled,
        neck: include.value.neck && !!props.config?.neck?.enabled,
      })
    } else {
      const action = kind.value === 'sequence'
        ? { name: key, sequence: { steps: rows.value.map(row => ({
          preset: row.preset, move_ms: row.move === '' ? undefined : ms(row.move), hold_ms: ms(row.hold),
        })) } }
        : { name: key, preset: { move_ms: ms(moveSeconds.value), targets: {
          ...editing.value?.action.preset?.targets, eye_expression,
        } } }
      saved = await remote.command('put_action', { expected_revision, action })
    }
    edit(saved)
    await load()
  } catch { /* 保留未保存的编辑内容。 */ }
  finally { busy.value = false }
}
async function remove() {
  const record = editing.value
  if (!record || busy.value || !confirm(`删除「${record.action.name}」？`)) return
  busy.value = true
  try {
    await remote.command('delete_action', { name: record.action.name, expected_revision: record.revision })
    edit(null)
    await load()
  } catch { /* 仍被序列使用的预设会由服务端指出。 */ }
  finally { busy.value = false }
}
function showExpression(value: string) {
  remote.targets({ eye_expression: value }, true)
}
onMounted(load)
watch(() => remote.online.value, online => { if (online) load() })
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center justify-between">
      <span class="text-sm text-muted-foreground">{{ running ? `正在执行：${running}` : '空闲' }}</span>
      <Button v-if="running" variant="destructive" :disabled="!remote.controlling.value" @click="remote.command('cancel_action').catch(() => {})"><Square /> 取消动作</Button>
      <Button v-else variant="ghost" size="sm" @click="load">刷新</Button>
    </div>

    <div v-if="props.config?.eyes?.enabled" class="grid grid-cols-3 gap-2" aria-label="表情">
      <Button v-for="item in expressions" :key="item.value" variant="outline" :disabled="!canRun" @click="showExpression(item.value)">{{ item.label }}</Button>
    </div>

    <section v-if="debug" class="space-y-4 rounded-xl border p-4">
      <div class="grid grid-cols-2 gap-2">
        <Button variant="outline" :disabled="busy" @click="edit(null, 'preset')"><Plus /> 新建预设</Button>
        <Button variant="outline" :disabled="busy" @click="edit(null, 'sequence')"><Plus /> 新建序列</Button>
      </div>
      <div class="space-y-2">
        <Label for="action-name">{{ kind === 'preset' ? '预设名称' : '序列名称' }}</Label>
        <Input id="action-name" v-model="name" maxlength="64" :readonly="!!editing" />
      </div>
      <template v-if="kind === 'preset'">
        <div class="flex flex-wrap gap-4 text-sm">
          <label v-if="props.config?.left_arm?.enabled"><input v-model="include.left_arm" type="checkbox" /> 左臂</label>
          <label v-if="props.config?.right_arm?.enabled"><input v-model="include.right_arm" type="checkbox" /> 右臂</label>
          <label v-if="props.config?.neck?.enabled"><input v-model="include.neck" type="checkbox" /> 头部</label>
        </div>
        <div class="grid grid-cols-2 gap-3">
          <label class="space-y-2 text-sm">运动时间（秒）<Input v-model.number="moveSeconds" type="number" min="0.001" max="9.999" step="0.1" /></label>
          <label v-if="props.config?.eyes?.enabled" class="space-y-2 text-sm">表情<select v-model="expression" :class="selectClass"><option value="">不切换表情</option><option v-for="item in expressions" :key="item.value" :value="item.value">{{ item.label }}</option></select></label>
        </div>
        <p class="text-xs text-muted-foreground">在舵机或头部页摆好姿态，再保存实际回读的位置。</p>
        <div class="flex flex-wrap gap-2">
          <Button :disabled="!remote.online.value || !name.trim() || busy" @click="save(true)">保存位置</Button>
          <Button v-if="editing" variant="outline" :disabled="!remote.online.value || busy" @click="save()">保存时间和表情</Button>
        </div>
      </template>
      <template v-else>
        <div v-for="(row, index) in rows" :key="row.id" class="space-y-2 rounded-lg bg-muted/50 p-3">
          <div class="flex items-center gap-2">
            <span class="text-sm">{{ index + 1 }}</span>
            <select v-model="row.preset" :aria-label="`第 ${index + 1} 步预设`" :class="selectClass">
              <option v-for="record in presets" :key="record.action.name" :value="record.action.name">{{ record.action.name }}</option>
            </select>
            <Button variant="ghost" size="icon" aria-label="上移" :disabled="index === 0" @click="reorder(index, -1)"><ArrowUp /></Button>
            <Button variant="ghost" size="icon" aria-label="下移" :disabled="index === rows.length - 1" @click="reorder(index, 1)"><ArrowDown /></Button>
            <Button variant="ghost" size="icon" aria-label="移除步骤" @click="rows.splice(index, 1)"><X /></Button>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <label class="space-y-1 text-sm">运动（秒）<Input v-model.number="row.move" type="number" min="0.001" max="9.999" step="0.1" placeholder="沿用预设" /></label>
            <label class="space-y-1 text-sm">停留（秒）<Input v-model.number="row.hold" type="number" min="0" max="600" step="0.1" /></label>
          </div>
        </div>
        <div class="flex gap-2">
          <Button variant="outline" :disabled="!presets.length" @click="addStep"><Plus /> 添加一步</Button>
          <Button :disabled="!remote.online.value || !name.trim() || !rows.length || busy" @click="save()">保存序列</Button>
        </div>
      </template>
      <Button v-if="editing" variant="ghost" class="text-destructive" :disabled="!remote.online.value || busy" @click="remove">删除{{ kind === 'preset' ? '预设' : '序列' }}</Button>
    </section>

    <p v-if="!active.length" class="py-6 text-center text-sm text-muted-foreground">暂无动作预设或序列</p>
    <ul class="flex flex-col gap-2">
      <li v-for="record in active" :key="record.action.name" class="flex gap-2">
        <Button variant="outline" class="h-12 flex-1 justify-between" :disabled="!canRun" @click="remote.command('run_action', { name: record.action.name }).catch(() => {})">
          <span class="truncate">{{ record.action.name }}</span>
          <span class="ml-auto text-xs text-muted-foreground">{{ record.action.preset ? '预设' : record.action.sequence ? record.action.sequence.steps.length + ' 步' : '动作' }}</span><Play />
        </Button>
        <Button v-if="debug && (record.action.preset || record.action.sequence)" variant="ghost" class="h-12" :disabled="busy" @click="edit(record)">编辑</Button>
      </li>
    </ul>
  </div>
</template>
