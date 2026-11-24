<script setup lang="ts">
import { computed } from 'vue'
import { PlugZap, Unplug } from 'lucide-vue-next'
import ActionButtonRail from '@/components/dashboard/ActionButtonRail.vue'
import ControlDeck from '@/components/dashboard/ControlDeck.vue'
import RobotModel from '@/components/dashboard/RobotModel.vue'
import SystemStatusCard from '@/components/dashboard/SystemStatusCard.vue'
import VideoStage from '@/components/dashboard/VideoStage.vue'
import { clientConfig } from '@/lib/settings'
import { useWanClient } from '@/lib/wan-client'

const emit = defineEmits<{ openConfig: [] }>()
const wan = useWanClient()

const title = computed(() => `IP机器人远程操作客户端`)
</script>

<template>
  <div class="dashboard-root dv-screen-bg">
    <!-- 传统大屏顶部标题装饰区 -->
    <header class="top-bar">
      <div class="top-left">
        <dv-decoration-8 class="deco-wing" :color="['#00d8ff', '#1d6fff']" />
        <!-- <div class="top-badges mono">
          <span class="dv-pill" :class="wan.signalOnline.value ? 'ok' : 'warn'">
            信令: {{ wan.signalOnline.value ? '在线' : '离线' }}
          </span>
          <span class="dv-pill" :class="wan.online.value ? 'ok' : 'warn'">
            RTC: {{ wan.online.value ? '已连接' : wan.phase.value }}
          </span>
        </div> -->
      </div>

      <div class="top-center">
        <div class="main-title">{{ title }}</div>
        <dv-decoration-5 class="deco-center" :color="['#00d8ff', '#1d6fff']" :dur="4" />
      </div>

      <div class="top-right">
        <div class="top-actions">
          <span v-if="wan.errorMessage.value" class="err-chip" @click="wan.errorMessage.value = ''">
            {{ wan.errorMessage.value }}
          </span>
          <button
            v-if="!wan.online.value"
            class="dv-btn dv-btn-sm dv-btn-ok"
            :disabled="wan.busy.value"
            @click="wan.connect()"
          >
            <PlugZap :size="13" /> 连接
          </button>
          <button
            v-else
            class="dv-btn dv-btn-sm dv-btn-warn"
            @click="wan.disconnect()"
          >
            <Unplug :size="13" /> 断开
          </button>
        </div>
        <dv-decoration-8 :reverse="true" class="deco-wing" :color="['#00d8ff', '#1d6fff']" />
      </div>
    </header>

    <!-- 对照草图 1：左通栏(状态+模型占位)、中上(图传)、右上(动作按钮)、下横栏(操控+角落配置按钮) -->
    <main class="grid-body">
      <aside class="area-left">
        <dv-border-box-12 :color="['#00d8ff', '#1d6fff']">
          <div class="left-inner">
            <RobotModel style="height: 50%;"/>
            <SystemStatusCard />
          </div>
        </dv-border-box-12>
      </aside>

      <section class="area-video">
        <VideoStage />
      </section>

      <aside class="area-actions">
        <ActionButtonRail />
      </aside>

      <section class="area-control">
        <ControlDeck @open-config="emit('openConfig')" />
      </section>
    </main>
  </div>
</template>

<style scoped>
.dashboard-root {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  padding: 6px 10px 10px;
  gap: 8px;
}

.top-bar {
  display: grid;
  grid-template-columns: 1fr 440px 1fr;
  align-items: center;
  height: 52px;
  flex-shrink: 0;
}

.top-left,
.top-right {
  position: relative;
  display: flex;
  align-items: center;
  height: 100%;
}

.top-right {
  justify-content: flex-end;
}

.deco-wing {
  width: 220px;
  height: 40px;
  opacity: 0.85;
}

.top-badges {
  display: flex;
  gap: 6px;
  margin-left: 8px;
}

.top-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-right: 8px;
}

.err-chip {
  max-width: 260px;
  padding: 3px 8px;
  border: 1px solid rgba(255, 59, 83, 0.7);
  background: rgba(140, 12, 30, 0.65);
  color: #ffb8c0;
  font-size: 11px;
  border-radius: 3px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}

.top-center {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
}

.main-title {
  font-size: 19px;
  font-weight: 800;
  letter-spacing: 0.14em;
  color: var(--cyan-bright);
  text-shadow: 0 0 12px rgba(0, 216, 255, 0.7);
  white-space: nowrap;
}

.deco-center {
  width: 400px;
  height: 20px;
}

.grid-body {
  display: grid;
  grid-template-columns: 280px 1fr 220px;
  grid-template-rows: 1fr 158px;
  gap: 8px;
  flex: 1;
  min-height: 0;
}

.area-left {
  grid-column: 1;
  grid-row: 1;
  min-height: 0;
}

.left-inner {
  display: flex;
  flex-direction: column;
  gap: 10px;
  height: 100%;
  padding: 14px 12px;
}

.area-video {
  grid-column: 2;
  grid-row: 1;
  min-height: 0;
}

.area-actions {
  display: grid;
  grid-template-rows: minmax(0, 1fr);
  gap: 8px;
  grid-column: 3;
  grid-row: 1;
  min-height: 0;
}

.area-control {
  grid-column: 1 / -1;
  grid-row: 2;
  min-height: 0;
}
</style>
