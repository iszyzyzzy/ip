<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import GazePad from '@/components/GazePad.vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useRemote } from '@/lib/remote'

const props = defineProps<{ config: Record<string, any> | null, revision: string }>()
const emit = defineEmits<{ saved: [] }>()
const remote = useRemote()
const draft = ref<Record<string, any> | null>(null)
const saving = ref(false)
const dirty = ref(false)
const saved = ref(false)
const baseRevision = ref('')
const previewState = ref('live')
const previewPhone = ref('')
let previewTimer = 0
let previewPending = false
let previewRequest: Promise<void> | null = null
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
const sliders = [
  { key: 'scale', label: '眼睛大小', min: 0.5, max: 1.5, step: 0.01, unit: '×' },
  { key: 'iris_size', label: '虹膜大小', min: 0.1, max: 0.22, step: 0.001, unit: '' },
  { key: 'pupil', label: '瞳孔大小', min: 0.2, max: 0.65, step: 0.01, unit: '' },
  { key: 'expression', label: '表情 · 放松 / 精神', min: -1, max: 1, step: 0.05, unit: '' },
]
watch(() => [props.config?.eyes?.settings, props.revision] as const, ([settings, revision]) => {
  if (settings && !dirty.value) {
    draft.value = clone(settings)
    if (!draft.value!.phone_ip) draft.value!.phone_ip = '192.168.3.110'
    baseRevision.value = revision
    previewPhone.value = settings.phone_ip ?? ''
  }
}, { immediate: true })
function setGaze(x: number, y: number) {
  if (!draft.value) return
  draft.value.simulator.target_x = x
  draft.value.simulator.target_y = y
  // 定睛应固定目标，而不是沿用模拟模式里的随机眼跳。
  draft.value.simulator.eye_motion = false
  changed()
}
function changed() {
  dirty.value = true; saved.value = false
  schedulePreview()
}
function phoneChanged() { dirty.value = true; saved.value = false }
function cancelPreview() { clearTimeout(previewTimer); previewTimer = 0; previewPending = false }
function schedulePreview() {
  if (!remote.online.value || !props.config?.eyes?.enabled || saving.value) return
  previewPending = true
  previewState.value = 'pending'
  if (!previewTimer && !previewRequest) previewTimer = window.setTimeout(flushPreview, 50)
}
function flushPreview() {
  clearTimeout(previewTimer); previewTimer = 0
  if (!previewPending || !draft.value || !remote.online.value || saving.value) return
  previewPending = false
  if (props.revision !== baseRevision.value) { previewState.value = 'conflict'; return }
  const settings = clone(draft.value)
  // 地址只在明确连接或保存时提交，避免输入中的半个IP打断外观预览。
  settings.phone_ip = previewPhone.value
  previewRequest = remote.command('update_eyes', { expected_revision: baseRevision.value, settings })
    .then(() => { previewState.value = 'live' })
    .catch(() => { cancelPreview(); previewState.value = 'error' })
    .finally(() => {
      previewRequest = null
      if (previewPending) schedulePreview()
    })
}
watch(() => remote.online.value, on => { if (!on) cancelPreview() })
onBeforeUnmount(cancelPreview)
function reset() {
  if (props.config?.eyes?.settings) draft.value = clone(props.config.eyes.settings)
  baseRevision.value = props.revision
  previewPhone.value = props.config?.eyes?.settings?.phone_ip ?? ''
  dirty.value = false; saved.value = false
  schedulePreview()
}
function color(value: any) {
  return '#' + ['r', 'g', 'b'].map(k => Math.round((value?.[k] ?? 1) * 255).toString(16).padStart(2, '0')).join('')
}
function setColor(key: string, value: string) {
  if (!draft.value) return
  draft.value[key] = Object.fromEntries(['r', 'g', 'b'].map((k, i) => [k, parseInt(value.slice(1 + i * 2, 3 + i * 2), 16) / 255]))
  changed()
}
const tracking = computed(() => {
  const eye = remote.state.value?.devices?.eyes
  if (!eye) return null
  return { tracking_live: eye.trackingLive ?? eye.tracking_live, tracking_packets: eye.trackingPackets ?? eye.tracking_packets,
    tracking_status: eye.trackingStatus ?? eye.tracking_status }
})
async function apply(face = false) {
  if (!draft.value || !props.config || saving.value) return
  if (props.revision !== baseRevision.value) {
    remote.message.value = '配置已在别处更新，请重新载入后调节'
    return
  }
  const settings = clone(draft.value)
  const ip = String(settings.phone_ip ?? '').trim()
  const validIp = /^\d{1,3}(\.\d{1,3}){3}$/.test(ip) && ip.split('.').every(v => Number(v) <= 255)
  if ((ip && !validIp) || ((face || settings.mode === 'IFACIALMOCAP') && !validIp)) {
    remote.message.value = '请输入有效的面捕手机IPv4地址'
    return
  }
  settings.phone_ip = ip
  if (face) settings.mode = 'IFACIALMOCAP'
  saving.value = true
  cancelPreview()
  try {
    // 等待已发出的预览结束，保证旧预览不会在保存之后覆盖新参数。
    await previewRequest
    const result = await remote.command<{ revision: string }>('update_eyes', {
      expected_revision: baseRevision.value, settings, save: !face, connect_tracking: face,
    })
    draft.value = settings
    previewPhone.value = ip
    previewState.value = 'live'
    if (!face) {
      baseRevision.value = result.revision
      dirty.value = false; saved.value = true
      emit('saved')
    } else { dirty.value = true; saved.value = false }
  } catch { previewState.value = 'error' }
  finally { saving.value = false }
}
function blink() { remote.targets({ blink: { left: true, right: true } }, true) }
</script>

<template>
  <div v-if="draft" class="flex flex-col gap-4">
    <div class="flex items-center justify-between text-xs text-muted-foreground"><span>实时预览 · 保存后下次启动沿用</span><span>{{ previewState === 'pending' ? '同步中…' : previewState === 'error' ? '同步失败' : previewState === 'conflict' ? '配置已变化，请重新载入' : '已同步' }}</span></div>
    <section class="space-y-4 rounded-xl border p-4">
      <div class="flex items-center justify-between">
        <h3 class="text-sm font-medium">渲染模式</h3>
        <span class="text-xs text-muted-foreground">{{ tracking?.tracking_live ? '面捕正在接收' : '自主运行不依赖手机' }}</span>
      </div>
      <ToggleGroup v-model="draft.mode" type="single" variant="outline" class="w-full" @update:model-value="v => { if (!v) draft!.mode = 'IDLE'; changed() }">
        <ToggleGroupItem value="IDLE" class="h-11 flex-1">自主</ToggleGroupItem>
        <ToggleGroupItem value="IFACIALMOCAP" class="h-11 flex-1">面捕</ToggleGroupItem>
        <ToggleGroupItem value="SIMULATOR" class="h-11 flex-1">定睛</ToggleGroupItem>
      </ToggleGroup>
      <div v-if="draft.mode === 'SIMULATOR'" class="space-y-3">
        <div class="flex items-center justify-between"><Label>注视位置</Label><Button variant="ghost" size="sm" @click="setGaze(0, 0)">回中</Button></div>
        <GazePad :x="draft.simulator.target_x" :y="draft.simulator.target_y" @change="setGaze" />
        <p class="text-xs text-muted-foreground">点击或拖动圆点，屏幕会实时跟随，松手保持。</p>
        <div class="flex items-center justify-between"><Label for="gaze-saccades">目标附近轻微眼动</Label><Switch id="gaze-saccades" v-model="draft.simulator.eye_motion" @update:model-value="changed" /></div>
      </div>
      <div class="space-y-2">
        <Label for="phone-ip">面捕手机 IP</Label>
        <div class="flex gap-2">
          <Input id="phone-ip" v-model="draft.phone_ip" placeholder="192.168.3.110" inputmode="decimal" class="h-11 min-w-0" @input="phoneChanged" />
          <Button variant="secondary" class="h-11" :disabled="!remote.online.value || saving || !props.config?.eyes?.enabled" @click="apply(true)">连接面捕</Button>
        </div>
        <p class="text-xs text-muted-foreground">手机打开 iFacialMocap，发送到树莓派 192.168.3.109 · UDP {{ draft.tracking_port }}。</p>
        <p v-if="tracking" class="text-xs text-muted-foreground">{{ tracking.tracking_status }} · 已收 {{ tracking.tracking_packets || 0 }} 包</p>
      </div>
    </section>

    <section class="space-y-5 rounded-xl border p-4">
      <h3 class="text-sm font-medium">外观</h3>
      <div v-for="s in sliders" :key="s.key" class="space-y-3">
        <div class="flex justify-between gap-3 text-sm"><Label :for="`eye-${s.key}`">{{ s.label }}</Label><span class="tabular-nums text-muted-foreground">{{ Number(draft[s.key]).toFixed(2) }}{{ s.unit }}</span></div>
        <Slider :id="`eye-${s.key}`" :model-value="[draft[s.key]]" :min="s.min" :max="s.max" :step="s.step" :aria-label="s.label" @update:model-value="v => { draft![s.key] = v![0]; changed() }" />
      </div>
      <div class="grid grid-cols-2 gap-3">
        <Label class="flex items-center justify-between gap-2 rounded-lg bg-muted/50 p-3">虹膜颜色<input aria-label="虹膜颜色" type="color" :value="color(draft.iris)" class="h-9 w-10 cursor-pointer border-0 bg-transparent" @input="setColor('iris', ($event.target as HTMLInputElement).value)" /></Label>
        <Label class="flex items-center justify-between gap-2 rounded-lg bg-muted/50 p-3">背景颜色<input aria-label="背景颜色" type="color" :value="color(draft.background)" class="h-9 w-10 cursor-pointer border-0 bg-transparent" @input="setColor('background', ($event.target as HTMLInputElement).value)" /></Label>
      </div>
    </section>

    <section class="space-y-5 rounded-xl border p-4">
      <div class="flex items-center justify-between"><Label for="auto-blink">自动眨眼</Label><Switch id="auto-blink" v-model="draft.auto_blink" @update:model-value="changed" /></div>
      <div v-if="draft.auto_blink" class="space-y-3">
        <div class="flex justify-between text-sm"><Label for="blink-interval">眨眼间隔</Label><span class="tabular-nums text-muted-foreground">{{ Number(draft.blink_interval).toFixed(1) }} 秒</span></div>
        <Slider id="blink-interval" :model-value="[draft.blink_interval]" :min="0.5" :max="20" :step="0.1" aria-label="眨眼间隔" @update:model-value="v => { draft!.blink_interval = v![0]; changed() }" />
        <div class="flex justify-between text-sm"><Label for="blink-duration">眨眼速度 · 快 / 慢</Label><span class="tabular-nums text-muted-foreground">{{ Number(draft.blink_duration).toFixed(2) }} 秒</span></div>
        <Slider id="blink-duration" :model-value="[draft.blink_duration]" :min="0.08" :max="1" :step="0.01" aria-label="眨眼时长" @update:model-value="v => { draft!.blink_duration = v![0]; changed() }" />
      </div>
      <div class="flex items-center justify-between"><Label for="eye-jelly">弹性晃动</Label><Switch id="eye-jelly" v-model="draft.jelly.enabled" @update:model-value="changed" /></div>
      <div v-if="draft.jelly.enabled" class="space-y-3">
        <Label for="jelly-strength">弹性强度</Label>
        <Slider id="jelly-strength" :model-value="[draft.jelly.strength]" :min="0" :max="1" :step="0.05" aria-label="弹性强度" @update:model-value="v => { draft!.jelly.strength = v![0]; changed() }" />
      </div>
      <Button variant="secondary" class="h-11 w-full" :disabled="!remote.controlling.value || !props.config?.eyes?.enabled" @click="blink">试一下眨眼</Button>
    </section>

    <p v-if="!props.config?.eyes?.enabled" class="rounded-lg bg-muted p-3 text-sm">眼屏尚未接入 robotd，接入后可实时调节。</p>
    <p class="text-xs text-muted-foreground">拖动即生效，保存后下次启动沿用；实时预览和保存都不会中断舵机或底盘控制。</p>
    <div class="grid grid-cols-2 gap-2">
      <Button variant="outline" class="h-11" :disabled="saving" @click="reset">重新载入</Button>
      <Button class="h-11" :disabled="!remote.online.value || saving" @click="apply()">{{ saving ? '应用中…' : saved ? '已保存参数' : '保存参数' }}</Button>
    </div>
  </div>
</template>
