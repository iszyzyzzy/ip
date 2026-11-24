<script setup lang="ts">
import { ref } from 'vue'
import {
  Activity,
  ArrowLeft,
  Camera,
  Eye,
  Gamepad2,
  GitBranch,
  Hand,
  OctagonX,
  PlugZap,
  Power,
  Sliders,
  Unlock,
} from 'lucide-vue-next'
import ConnectionConfig from '@/components/config/ConnectionConfig.vue'
import DiagnosticsPanel from '@/components/config/DiagnosticsPanel.vue'
import GamepadBindingPanel from '@/components/config/GamepadBindingPanel.vue'
import HeadEyesPanel from '@/components/config/HeadEyesPanel.vue'
import PresetManager from '@/components/config/PresetManager.vue'
import SequenceEditor from '@/components/config/SequenceEditor.vue'
import ServoDebugPanel from '@/components/config/ServoDebugPanel.vue'
import { useWanClient } from '@/lib/wan-client'

const emit = defineEmits<{ back: [] }>()
const wan = useWanClient()

type SectionId = 'connection' | 'presets' | 'sequences' | 'servo' | 'head' | 'gamepad' | 'diagnostics'
const section = ref<SectionId>('connection')

const navItems: { id: SectionId, label: string, icon: any }[] = [
  { id: 'connection', label: '连接与系统设置', icon: PlugZap },
  { id: 'presets', label: '动作预设管理', icon: Camera },
  { id: 'sequences', label: '序列与补插曲线', icon: GitBranch },
  { id: 'servo', label: '手动舵机与标定', icon: Sliders },
  { id: 'head', label: '头部与眼屏调试', icon: Eye },
  { id: 'gamepad', label: '车端蓝牙手柄', icon: Gamepad2 },
  { id: 'diagnostics', label: '状态诊断与日志', icon: Activity },
]
</script>

<template>
  <div class="console-root dv-screen-bg">
    <!-- 顶部全局安全条与返回大屏按钮 -->
    <header class="console-header">
      <button class="dv-btn" @click="emit('back')">
        <ArrowLeft :size="15" /> 返回遥操作大屏
      </button>

      <div class="header-title">机器人系统配置与动作曲线编排后台</div>

      <div class="header-right">
        <span class="dv-pill mono" :class="wan.online.value ? 'ok' : 'warn'">
          {{ wan.online.value ? 'WebRTC 在线' : wan.statusMessage.value }}
        </span>
        <button
          class="dv-btn dv-btn-sm"
          :class="wan.controlling.value ? 'dv-btn-warn' : 'dv-btn-ok'"
          :disabled="!wan.online.value || wan.busy.value"
          @click="wan.controlling.value ? wan.release() : wan.acquire()"
        >
          <template v-if="wan.controlling.value"><Power :size="13" /> 释放控制</template>
          <template v-else-if="wan.state.value?.latched"><Unlock :size="13" /> 解锁控制</template>
          <template v-else><Hand :size="13" /> 取得控制</template>
        </button>
        <button
          class="dv-btn dv-btn-sm dv-btn-danger"
          :disabled="!wan.online.value"
          @click="wan.emergencyStop()"
        >
          <OctagonX :size="14" /> 急停
        </button>
      </div>
    </header>

    <!-- 对照草图 2：左侧导航栏 + 右侧主要内容区 -->
    <div class="console-body">
      <aside class="console-nav">
        <dv-border-box-12 :color="['#00d8ff', '#1d6fff']">
          <nav class="nav-inner">
            <div class="dv-section-title">
              <span>配置导航</span>
            </div>
            <button
              v-for="item in navItems"
              :key="item.id"
              class="dv-btn nav-item"
              :class="{ active: section === item.id }"
              @click="section = item.id"
            >
              <component :is="item.icon" :size="15" />
              <span>{{ item.label }}</span>
            </button>
          </nav>
        </dv-border-box-12>
      </aside>

      <main class="console-main">
        <dv-border-box-13 :color="['#00d8ff', '#1d6fff']">
          <div class="main-inner">
            <ConnectionConfig v-if="section === 'connection'" />
            <PresetManager v-else-if="section === 'presets'" />
            <SequenceEditor v-else-if="section === 'sequences'" />
            <ServoDebugPanel v-else-if="section === 'servo'" />
            <HeadEyesPanel v-else-if="section === 'head'" />
            <GamepadBindingPanel v-else-if="section === 'gamepad'" />
            <DiagnosticsPanel v-else />
          </div>
        </dv-border-box-13>
      </main>
    </div>
  </div>
</template>

<style scoped>
.console-root {
  display: flex;
  flex-direction: column;
  width: 100vw;
  height: 100vh;
  padding: 8px 12px 12px;
  gap: 10px;
}

.console-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  height: 46px;
  padding: 0 10px;
  border: 1px solid rgba(0, 216, 255, 0.28);
  border-radius: 4px;
  background: rgba(6, 22, 52, 0.85);
  flex-shrink: 0;
}

.header-title {
  font-size: 16px;
  font-weight: 700;
  color: var(--cyan-bright);
  letter-spacing: 0.08em;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.console-body {
  display: grid;
  grid-template-columns: 230px 1fr;
  gap: 10px;
  flex: 1;
  min-height: 0;
}

.console-nav,
.console-main {
  min-height: 0;
}

.nav-inner {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  padding: 14px 12px;
}

.nav-item {
  width: 100%;
  justify-content: flex-start;
  padding: 10px 12px;
  font-size: 13px;
}

.main-inner {
  height: 100%;
  padding: 16px 14px 14px;
  min-height: 0;
}
</style>
