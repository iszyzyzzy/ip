<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { Play, RefreshCw, Square } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import { type ActionRecord, useRemote } from '@/lib/remote'

const remote = useRemote()
const actions = ref<ActionRecord[]>([])
const loading = ref(false)

async function load() {
  if (!remote.online.value) return
  loading.value = true
  try {
    const result = await remote.command<{ actions: ActionRecord[] }>('list_actions')
    actions.value = result.actions.filter(a => !a.deleted)
  } catch { /* 错误已写入全局提示 */ } finally {
    loading.value = false
  }
}

async function run(name: string) {
  await remote.command('run_action', { name }).catch(() => {})
}

async function cancel() {
  await remote.command('cancel_action').catch(() => {})
}

onMounted(load)
watch(() => remote.online.value, on => on && load())
</script>

<template>
  <div class="flex flex-col gap-3">
    <div class="flex items-center justify-between">
      <span class="text-sm text-muted-foreground">
        {{ remote.state.value?.action ? `正在执行：${remote.state.value.action}` : '空闲' }}
      </span>
      <Button variant="ghost" size="icon" aria-label="刷新动作列表" :disabled="loading" @click="load">
        <RefreshCw :class="loading && 'animate-spin'" />
      </Button>
    </div>

    <Button
      v-if="remote.state.value?.action"
      variant="destructive"
      class="h-12"
      :disabled="!remote.controlling.value"
      @click="cancel"
    >
      <Square /> 取消动作
    </Button>

    <p v-if="!actions.length" class="py-8 text-center text-sm text-muted-foreground">
      暂无动作，可用 robotctl put-action 添加
    </p>
    <ul class="flex flex-col gap-2">
      <li v-for="a in actions" :key="a.action.name">
        <Button
          variant="outline"
          class="h-14 w-full justify-between px-4"
          :disabled="!remote.controlling.value || !!remote.state.value?.action"
          @click="run(a.action.name)"
        >
          <span class="flex flex-col items-start">
            <span class="font-medium">{{ a.action.name }}</span>
            <span class="text-xs text-muted-foreground">
              {{ ((a.action.duration_ms ?? 0) / 1000).toFixed(1) }} s · {{ a.action.steps?.length ?? 0 }} 步
            </span>
          </span>
          <Play />
        </Button>
      </li>
    </ul>
  </div>
</template>
