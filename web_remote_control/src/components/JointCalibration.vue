<script setup lang="ts">
import { computed, ref } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useRemote } from '@/lib/remote'

const props = defineProps<{ config: Record<string, any>, revision: string, side: string, index: number, reading: number | null }>()
const emit = defineEmits<{ saved: [] }>()
const remote = useRemote()
const fields = { low: '最小读数', zero: '零位读数', high: '最大读数' } as const
const draft = ref({ low: 0, zero: 0, high: 0 })
const open = ref(false)
const saving = ref(false)
const revision = ref('')
const joint = computed(() => props.config[props.side].joints[props.index])
function edit() {
  const j = joint.value
  const ends = [j.min_rad, j.max_rad].map(a => Math.round(j.center_pwm + a * j.direction * j.pwm_per_rad))
  draft.value = { low: Math.min(...ends), zero: j.center_pwm, high: Math.max(...ends) }
  revision.value = props.revision
  open.value = true
}
const problem = computed(() => {
  if (props.revision !== revision.value) return '配置已更新，请取消后重新打开标定。'
  const { low, zero, high } = draft.value
  if (![low, zero, high].every(v => Number.isInteger(v) && v >= 500 && v <= 2500)) return '读数需为 500～2500 的整数。'
  if (low >= high || zero < low || zero > high) return '最小读数应小于最大读数，零位应在两者之间。'
  if (Math.max(high - zero, zero - low) > Math.PI * joint.value.pwm_per_rad) return '端点距零位超过 180°。'
  return ''
})
async function save() {
  if (saving.value || problem.value || !remote.controlling.value) return
  saving.value = true
  try {
    const config = JSON.parse(JSON.stringify(props.config))
    const j = config[props.side].joints[props.index]
    const { low, zero, high } = draft.value
    const ends = [low, high].map(p => (p - zero) / (j.direction * j.pwm_per_rad))
    Object.assign(j, { center_pwm: zero, min_rad: Math.min(...ends), max_rad: Math.max(...ends) })
    await remote.command('update_config', { expected_revision: revision.value, config })
    emit('saved')
    open.value = false
    remote.message.value = '关节标定已保存'
  } catch { /* 网关返回的限位或版本错误由页面显示。 */ }
  finally { saving.value = false }
}
</script>

<template>
  <Button v-if="!open" variant="outline" class="h-11 w-full" @click="edit">标定可动范围</Button>
  <fieldset v-else class="space-y-4 border-t pt-4" :disabled="saving">
    <legend class="text-sm font-medium">关节标定</legend>
    <div v-for="(label, key) in fields" :key="key" class="flex items-end gap-3">
      <label class="flex-1 space-y-1 text-sm">{{ label }}
        <Input v-model.number="draft[key]" type="number" min="500" max="2500" step="1" class="h-11" />
      </label>
      <Button variant="outline" class="h-11" :disabled="reading == null" @click="reading != null && (draft[key] = reading)">取实际值</Button>
    </div>
    <p class="text-xs text-muted-foreground">保存会停止当前动作并释放控制权。已有动作按新零位换算，手动调试仍可发送原始位置。</p>
    <p v-if="problem" class="text-sm text-destructive">{{ problem }}</p>
    <div class="flex justify-end gap-3">
      <Button variant="ghost" @click="open = false">取消</Button>
      <Button :disabled="!!problem || !remote.controlling.value || saving" @click="save">保存关节标定</Button>
    </div>
  </fieldset>
</template>
