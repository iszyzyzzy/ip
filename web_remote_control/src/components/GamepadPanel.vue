<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Button } from '@/components/ui/button'
import { type ActionRecord, useRemote } from '@/lib/remote'

const props = defineProps<{ active: boolean }>()
const remote = useRemote()
const modifier = ref('RB')
const button = ref('A')
const chord = computed(() => modifier.value + '+' + button.value)
const actions = ref<string[]>([])
const bindings = ref<Record<string, string>>({})
const action = ref('')
const pending = ref(false)
const loaded = ref(false)
const selected = computed(() => bindings.value[chord.value] || '')
function selectBinding(key: string) {
  const [mod, face] = key.split('+')
  if (mod && face) { modifier.value = mod; button.value = face }
}
watch([chord, bindings], () => { action.value = selected.value })
watch(() => [props.active, remote.online.value], async () => {
  if (!props.active || !remote.online.value) return
  loaded.value = false
  try {
    const [pad, library] = await Promise.all([
      remote.command('get_gamepad'), remote.command<{actions: ActionRecord[]}>('list_actions'),
    ])
    bindings.value = pad.bindings
    actions.value = library.actions.filter(r => !r.deleted).map(r => r.action.name)
    loaded.value = true
  } catch { /* 网关显示具体错误。 */ }
}, { immediate: true })

async function save() {
  pending.value = true
  try {
    const result = await remote.command('set_gamepad', { chord: chord.value, action: action.value })
    bindings.value = result.bindings
  } catch { /* 保留尚未保存的选择。 */ }
  finally { pending.value = false }
}
</script>

<template>
  <div class="space-y-4">
    <div class="rounded-lg border p-3 text-sm" role="status">
      <strong>{{ remote.online.value && remote.gamepad.value?.device || '手柄未连接' }}</strong>
      <span v-if="remote.gamepad.value?.controlling"> · 控制中</span>
      <p v-if="remote.gamepad.value?.error" class="text-destructive">{{ remote.gamepad.value.error }}</p>
    </div>
    <p class="text-sm leading-relaxed">摇杆回中后按 Menu 取得或释放控制权。按住 LB，用左摇杆前后行驶、左右转向；松开 LB 驻车。View 急停。</p>
    <form class="space-y-4 rounded-xl border p-4" @submit.prevent="save">
      <h2 class="font-medium">动作绑定</h2>
      <div class="grid grid-cols-2 gap-3">
        <label class="text-sm">按住<select v-model="modifier"><option v-for="m in ['RB','L3','R3']" :key="m">{{ m }}</option></select></label>
        <label class="text-sm">再按<select v-model="button"><option v-for="b in ['A','B','X','Y']" :key="b">{{ b }}</option></select></label>
      </div>
      <label class="block text-sm">动作或序列<select v-model="action">
        <option value="">无绑定</option>
        <option v-if="selected && !actions.includes(selected)" :value="selected">{{ selected }}（已移除）</option>
        <option v-for="a in actions" :key="a">{{ a }}</option>
      </select></label>
      <Button :disabled="pending || !loaded || !remote.online.value || selected === action">保存绑定</Button>
    </form>
    <ul class="divide-y text-sm"><li v-for="(name, key) in bindings" :key="key">
      <button class="flex w-full justify-between py-3" @click="selectBinding(key)"><span>{{ key }}</span><span>{{ name }}</span></button>
    </li></ul>
  </div>
</template>

<style scoped>
select { display: block; width: 100%; margin-top: .35rem; padding: .6rem; border: 1px solid var(--border); border-radius: .5rem; background: var(--background); }
</style>
