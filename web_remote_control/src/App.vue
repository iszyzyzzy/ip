<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { Hand, OctagonX, Power, Unlock } from '@lucide/vue'
import ActionsPanel from '@/components/ActionsPanel.vue'
import ChassisPanel from '@/components/ChassisPanel.vue'
import HeadPanel from '@/components/HeadPanel.vue'
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
const tokenInput = ref('')
const now = ref(Date.now())
let clock = 0

// 配置只用于读取限位；变更后 config_revision 增加，随之重新读取。
async function loadConfig() {
  try {
    const result = await remote.command<{ config: Record<string, any> }>('get_config')
    config.value = result.config
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
  <div class="mx-auto flex min-h-dvh max-w-xl flex-col bg-background pb-[env(safe-area-inset-bottom)]">
    <header class="sticky top-0 z-10 flex items-center gap-2 border-b bg-background/95 px-4 pb-3 pt-[max(0.75rem,env(safe-area-inset-top))] backdrop-blur">
      <h1 class="text-base font-semibold">
        {{ config?.metadata?.name || '机器人' }}遥控
      </h1>
      <Badge :variant="status.tone">{{ status.text }}</Badge>
      <Badge v-if="remote.state.value?.simulated" variant="outline">模拟</Badge>
      <Button
        class="ml-auto h-10 px-3"
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
        已锁定停止。确认安全后点击“解锁并控制”。
      </p>

      <Tabs default-value="chassis" orientation="horizontal" class="flex-1 flex-col">
        <TabsList class="grid h-11 w-full grid-cols-4">
          <TabsTrigger value="chassis">底盘</TabsTrigger>
          <TabsTrigger value="head">头部</TabsTrigger>
          <TabsTrigger value="actions">动作</TabsTrigger>
          <TabsTrigger value="status">状态</TabsTrigger>
        </TabsList>
        <!-- 保持挂载：头部摇杆显示的位置须与已下发的颈部目标一致，切换标签不能重置。 -->
        <TabsContent value="chassis" class="pt-3" force-mount>
          <ChassisPanel :config="config" />
        </TabsContent>
        <TabsContent value="head" class="pt-3" force-mount>
          <HeadPanel :config="config" />
        </TabsContent>
        <TabsContent value="actions" class="pt-3" force-mount>
          <ActionsPanel />
        </TabsContent>
        <TabsContent value="status" class="pt-3" force-mount>
          <StatusPanel />
        </TabsContent>
      </Tabs>
    </main>

    <!-- 急停固定在拇指区，无需控制权即可触发，任何连接状态下都显示。 -->
    <footer class="sticky bottom-0 border-t bg-background/95 p-4 pb-[max(1rem,env(safe-area-inset-bottom))] backdrop-blur">
      <Button
        variant="destructive"
        class="h-16 w-full text-lg font-semibold"
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
