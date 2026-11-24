<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useInputController } from '@/lib/input-controller'
import { loadClientConfig } from '@/lib/settings'
import { cleanupSignalBridge } from '@/lib/signal'
import { useWanClient } from '@/lib/wan-client'
import ConfigConsole from '@/views/ConfigConsole.vue'
import TeleopDashboard from '@/views/TeleopDashboard.vue'

const view = ref<'dashboard' | 'config'>('dashboard')
const wan = useWanClient()
const input = useInputController()

async function switchView(next: 'dashboard' | 'config') {
  if (next === 'config') {
    // 切进后台前先停止本地按键/摇杆输出，避免切界面时还维持移动。
    input.stopAllInputs()
  }
  view.value = next
  await nextTick()
  // DataV 边框在隐藏状态尺寸为 0，恢复显示后主动通知重新测量 SVG 尺寸。
  window.dispatchEvent(new Event('resize'))
}

onMounted(async () => {
  await loadClientConfig()
  input.mount()
})

onUnmounted(() => {
  input.unmount()
  wan.disconnect().catch(() => {})
  cleanupSignalBridge()
})
</script>

<template>
  <TeleopDashboard v-show="view === 'dashboard'" @open-config="switchView('config')" />
  <ConfigConsole v-show="view === 'config'" @back="switchView('dashboard')" />
</template>
