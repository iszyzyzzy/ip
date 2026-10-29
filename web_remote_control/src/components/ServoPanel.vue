<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'
import { useRemote } from '@/lib/remote'

const props = defineProps<{ config: Record<string, any> | null }>()
const remote = useRemote()
const side = ref('left_arm')
const drafts = reactive<Record<string, number>>({})
const sending = ref(false)
const arm = computed(() => props.config?.[side.value])
const rows = computed(() => (arm.value?.joints ?? []).map((joint: any, index: number) => {
  const feedback = remote.state.value?.devices?.[side.value]?.[String(index)]
  const fresh = feedback && !feedback.error && Number.isFinite(feedback.pwm)
    && Date.now() / 1000 - feedback.feedback_time < 2
  return { id: joint.id, index, key: `${side.value}-${index}`, feedback, fresh }
}))
watch(rows, values => {
  for (const row of values) {
    if (row.fresh && drafts[row.key] == null) drafts[row.key] = row.feedback.pwm
  }
}, { immediate: true })

async function move(index: number, value: number) {
  if (!remote.controlling.value || sending.value || !arm.value?.enabled || !rows.value[index]?.fresh) return
  if (!Number.isInteger(value) || value < 500 || value > 2500) {
    remote.message.value = '位置需为500～2500的整数'
    return
  }
  sending.value = true
  try {
    // 只发送选中的舵机，不拿其他关节的旧反馈拼成整臂目标。
    await remote.command('targets', { targets: { servo: {
      right_arm: side.value === 'right_arm', joint_index: index, position_pwm: value,
    } } })
  } catch { /* 错误由统一提示显示，运动指令不重试。 */ }
  finally { sending.value = false }
}
function nudge(index: number, delta: number) {
  const row = rows.value[index]
  if (!row?.fresh) return
  drafts[row.key] = Math.max(500, Math.min(2500, row.feedback.pwm + delta))
  move(index, drafts[row.key]!)
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <ToggleGroup v-model="side" type="single" variant="outline" class="w-full" @update:model-value="v => !v && (side = 'left_arm')">
      <ToggleGroupItem value="left_arm" class="h-11 flex-1">左臂 · U2</ToggleGroupItem>
      <ToggleGroupItem value="right_arm" class="h-11 flex-1">右臂 · U3</ToggleGroupItem>
    </ToggleGroup>
    <p class="text-xs leading-relaxed text-muted-foreground">从根部到末端逐个调节。拖动滑条后松手发送位置，或输入读数后点击移动；运动时间固定为 0001，不改标定零点。</p>
    <p v-if="!arm?.enabled" class="rounded-lg bg-muted p-3 text-sm">这一侧舵机尚未启用。</p>
    <section v-for="row in rows" :key="row.key" class="space-y-4 rounded-xl border p-4">
      <div class="flex items-center justify-between gap-3">
        <h3 class="text-sm font-medium">关节 {{ row.index + 1 }} <span class="ml-1 text-xs text-muted-foreground">ID {{ row.id }}</span></h3>
        <span class="text-sm tabular-nums" :class="row.fresh ? '' : 'text-muted-foreground'">{{ row.fresh ? `实际 ${row.feedback.pwm}` : row.feedback?.error || '等待反馈' }}</span>
      </div>
      <Slider :model-value="[drafts[row.key] ?? 1500]" :min="500" :max="2500" :step="1"
        :aria-label="`关节${row.index + 1}目标位置`"
        :disabled="!remote.controlling.value || !arm?.enabled || !row.fresh || sending"
        @update:model-value="v => drafts[row.key] = v![0]!" @value-commit="v => move(row.index, v[0]!)" />
      <div class="flex items-center gap-2">
        <Button variant="outline" class="h-10" :disabled="!remote.controlling.value || !row.fresh || sending" @click="nudge(row.index, -10)">−10</Button>
        <Input v-model.number="drafts[row.key]" type="number" min="500" max="2500" step="1" :aria-label="`关节${row.index + 1}目标读数`" class="h-10 min-w-0 text-center tabular-nums" />
        <Button variant="outline" class="h-10" :disabled="!remote.controlling.value || !row.fresh || sending" @click="nudge(row.index, 10)">+10</Button>
        <Button class="h-10" :disabled="!remote.controlling.value || !row.fresh || sending || !arm?.enabled" @click="move(row.index, drafts[row.key]!)">移动</Button>
      </div>
    </section>
    <p v-if="!remote.controlling.value" class="text-center text-xs text-muted-foreground">取得控制权后即可调节。</p>
  </div>
</template>
