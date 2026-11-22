<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Hand, OctagonX, Power, Unlock } from '@lucide/vue'
import OfflinePanel from '@/components/OfflinePanel.vue'
import GamepadPanel from '@/components/GamepadPanel.vue'
import CompanionPanel from '@/components/CompanionPanel.vue'
import SpeakerPanel from '@/components/SpeakerPanel.vue'
import ActionsPanel from '@/components/ActionsPanel.vue'
import ChassisPanel from '@/components/ChassisPanel.vue'
import HeadPanel from '@/components/HeadPanel.vue'
import ServoPanel from '@/components/ServoPanel.vue'
import EyesPanel from '@/components/EyesPanel.vue'
import StatusPanel from '@/components/StatusPanel.vue'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useRemote } from '@/lib/remote'

const remote = useRemote()
const config = ref<Record<string, any> | null>(null)
const configRevision = ref('')
const mode = ref<'operate' | 'debug'>('operate')
const tab = ref('chassis')
const tokenInput = ref('')
watch(mode, value => { tab.value = value === 'debug' ? 'servo' : 'chassis' })
const companion = ref(localStorage.getItem('robot-remote-view') === 'companion')
async function switchView() {
  if (!companion.value) await remote.release()
  companion.value = !companion.value
  mode.value = 'operate'
  localStorage.setItem('robot-remote-view', companion.value ? 'companion' : 'full')
}
const now = ref(Date.now())
let clock = 0

// 以robotd版本为准，防止眼屏参数覆盖别处刚更新的配置。
async function loadConfig() {
  try {
    const result = await remote.command<{ config: Record<string, any>, revision: string }>('get_config')
    config.value = result.config
    configRevision.value = result.revision
  } catch { /* 提示已显示，摇杆回退到默认限位 */ }
}

watch(() => remote.online.value, on => on && loadConfig())
watch(() => remote.state.value?.config_revision, (rev, old) => {
  if (old && rev !== old) loadConfig()
})

onMounted(() => {
  remote.connect()
  clock = window.setInterval(() => (now.value = Date.now()), 500)
})
onUnmounted(() => clearInterval(clock))

const stale = computed(() => remote.online.value && now.value - remote.lastStateAt.value > 1500)

const available = computed(() => {
  const state = remote.state.value
  const devices = state?.devices
  const live = remote.online.value && !!state && !stale.value && !remote.backendError.value && !state.fault
  const ages = devices?.chassis?.feedback_age_ms
  const enabled = (name: string) => live && !!config.value?.[name]?.enabled
  const fresh = (age: unknown, limit: number) => typeof age === 'number' && age <= limit
  const chassis = ['system', 'mode'].every(k => fresh(ages?.[k], config.value?.chassis?.feedback_timeout_ms ?? 300))
  const neck = ['gm6020', 'dm4310'].every(k => fresh(devices?.neck?.[k]?.feedback_age_ms, 500))
  return {
    live, chassis: enabled('chassis') && (state?.simulated || chassis),
    neck: enabled('neck') && (state?.simulated || neck),
    eyes: enabled('eyes') && (state?.simulated || !!devices?.eyes?.config),
  }
})
const status = computed(() => {
  if (!remote.online.value) return { text: remote.link.value === 'connecting' ? '连接中' : '离线', tone: 'secondary' as const }
  if (remote.backendError.value) return { text: 'robotd 故障', tone: 'destructive' as const }
  if (stale.value) return { text: '状态延迟', tone: 'destructive' as const }
  const s = remote.state.value
  if (s?.fault) return { text: '设备故障', tone: 'destructive' as const }
  if (remote.controlling.value) return { text: '控制中', tone: 'default' as const }
  if (s?.owner) return { text: `被 ${s.owner} 占用`, tone: 'secondary' as const }
  return { text: '已连接', tone: 'outline' as const }
})

const latched = computed(() => !!remote.state.value?.latched)

function submitToken() {
  remote.setToken(tokenInput.value.trim())
  tokenInput.value = ''
}
</script>

<template>
  <div :class="companion ? 'max-w-5xl' : 'max-w-xl'" class="mx-auto flex min-h-dvh w-full flex-col bg-background pb-[env(safe-area-inset-bottom)]">
    <header class="sticky top-0 z-10 flex flex-wrap items-center gap-2 border-b bg-background/95 px-4 pb-3 pt-[max(0.75rem,env(safe-area-inset-top))] backdrop-blur">
      <h1 class="text-base font-semibold">
        {{ config?.metadata?.name || '机器人' }}遥控
      </h1>
      <Button v-if="!companion" variant="ghost" size="sm" class="h-7 px-2 text-xs" @click="mode = mode === 'debug' ? 'operate' : 'debug'">{{ mode === 'debug' ? '退出调试' : '进入调试模式' }}</Button>
      <Button variant="outline" size="sm" class="h-7 px-2 text-xs" :disabled="remote.busy.value" @click="switchView">{{ companion ? '全控遥控' : '手柄仪表' }}</Button>
      <Badge :variant="status.tone">{{ status.text }}</Badge>
      <Badge v-if="remote.state.value?.simulated" variant="outline">模拟</Badge>
      <Button
        v-if="!companion" class="ml-auto h-10 px-3"
        :variant="remote.controlling.value ? 'secondary' : 'default'"
        :disabled="!remote.online.value || remote.busy.value"
        @click="remote.controlling.value ? remote.release() : remote.acquire()"
      >
        <template v-if="remote.controlling.value"><Power /> 释放</template>
        <template v-else-if="latched"><Unlock /> 解锁并控制</template>
        <template v-else><Hand /> 取得控制</template>
      </Button>
    </header>

    <main class="flex flex-1 flex-col gap-4 p-4">
      <Card v-if="remote.needToken.value">
        <CardHeader>
          <CardTitle>需要访问凭据</CardTitle>
          <CardDescription>网关启用了 ROBOT_REMOTE_API_TOKEN，凭据只保存在此浏览器。</CardDescription>
        </CardHeader>
        <CardContent>
          <form class="flex flex-col gap-3" @submit.prevent="submitToken">
            <Label for="token">凭据</Label>
            <Input id="token" v-model="tokenInput" type="password" autocomplete="current-password" class="h-11" />
            <Button type="submit" class="h-11" :disabled="!tokenInput.trim()">连接</Button>
          </form>
        </CardContent>
      </Card>

      <p
        v-if="remote.message.value"
        role="alert"
        class="rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive"
        @click="remote.message.value = ''"
      >
        {{ remote.message.value }}
      </p>
      <p v-if="remote.backendError.value" class="rounded-lg bg-destructive/10 px-3 py-2 text-sm text-destructive">
        robotd：{{ remote.backendError.value.message }}
      </p>
      <p v-if="latched && !remote.controlling.value" class="rounded-lg bg-amber-500/10 px-3 py-2 text-sm text-amber-700 dark:text-amber-400">
        {{ companion ? '已急停锁定，请在全控遥控中解锁。' : '已锁定停止。确认安全后点击“解锁并控制”。' }}
      </p>

      <CompanionPanel v-if="companion" :chassis-online="!!available.chassis" :live="available.live" />
      <Tabs v-else v-model="tab" orientation="horizontal" class="flex-1 flex-col">
        <TabsList class="grid h-11 w-full" :class="mode === 'debug' ? 'grid-cols-6' : 'grid-cols-4'">
          <TabsTrigger v-if="mode === 'operate'" value="chassis">底盘</TabsTrigger>
          <TabsTrigger value="head">头部</TabsTrigger>
          <TabsTrigger v-if="mode === 'debug'" value="servo">手动</TabsTrigger>
          <TabsTrigger v-if="mode === 'debug'" value="eyes">眼屏</TabsTrigger>
          <TabsTrigger value="actions">动作</TabsTrigger>
          <TabsTrigger v-if="mode === 'debug'" value="gamepad">手柄</TabsTrigger>
          <TabsTrigger value="status">状态</TabsTrigger>
        </TabsList>
        <!-- 保持挂载：头部摇杆显示的位置须与已下发的颈部目标一致，切换标签不能重置。 -->
        <TabsContent v-if="mode === 'operate'" value="chassis" class="pt-3" force-mount>
          <OfflinePanel name="底盘" :offline="!available.chassis"><ChassisPanel :config="config" :active="!!available.chassis && tab === 'chassis'" /></OfflinePanel>
        </TabsContent>
        <TabsContent value="head" class="pt-3" force-mount>
          <OfflinePanel name="头部" :offline="!available.neck && !available.eyes"><HeadPanel :config="config" :neck-online="!!available.neck" :eyes-online="!!available.eyes" :debug="mode === 'debug'" /></OfflinePanel>
        </TabsContent>
        <TabsContent v-if="mode === 'debug'" value="servo" class="pt-3" force-mount>
          <ServoPanel :live="available.live" :config="config" :revision="configRevision" @saved="loadConfig" />
        </TabsContent>
        <TabsContent v-if="mode === 'debug'" value="eyes" class="pt-3" force-mount>
          <OfflinePanel name="眼屏" :offline="!available.eyes"><EyesPanel :config="config" :revision="configRevision" @saved="loadConfig" /></OfflinePanel>
        </TabsContent>
        <TabsContent value="actions" class="pt-3" force-mount>
          <OfflinePanel name="robotd " :offline="!available.live"><ActionsPanel :eyes-online="!!available.eyes" :debug="mode === 'debug'" :config="config" /></OfflinePanel>
        </TabsContent>
        <TabsContent v-if="mode === 'debug'" value="gamepad" class="pt-3"><GamepadPanel :active="tab === 'gamepad'" /></TabsContent>
        <TabsContent value="status" class="pt-3" force-mount>
          <StatusPanel />
        </TabsContent>
      </Tabs>
      <SpeakerPanel />
    </main>

    <!-- 急停固定在拇指区，无需控制权即可触发，任何连接状态下都显示。 -->
    <footer class="sticky bottom-0 z-10 border-t bg-background/95 p-4 pb-[max(1rem,env(safe-area-inset-bottom))] backdrop-blur">
      <Button
        variant="destructive"
        class="w-full text-lg font-semibold" :class="companion ? 'h-10' : 'h-16'"
        :disabled="!remote.online.value"
        @click="remote.stop(true)"
      >
        <OctagonX class="size-6" /> 急停
      </Button>
    </footer>
  </div>
</template>

<style>
/* force-mount 的非激活面板需隐藏；reka 会给它加 hidden 属性但 Tailwind 的 flex 可能覆盖。 */
[data-slot='tabs-content'][data-state='inactive'] { display: none; }
</style>
