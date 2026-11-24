<script setup lang="ts">
import { computed } from 'vue'
import { Play, RefreshCw, Square } from 'lucide-vue-next'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()

const activeActions = computed(() => wan.actions.value.filter(r => !r.deleted))
const runningName = computed(() => wan.state.value?.action || '')
const canRun = computed(() => wan.controlling.value && !runningName.value)
function triggerAction(name: string) {
  wan.runAction(name).catch(err => {
    wan.errorMessage.value = String((err as Error).message || err)
  })
}

function cancel() {
  wan.cancelAction().catch(err => {
    wan.errorMessage.value = String((err as Error).message || err)
  })
}
</script>

<template>
  <div class="action-rail">
    <dv-border-box-13 :color="['#00d8ff', '#1d6fff']">
      <div class="rail-inner">
        <div class="dv-section-title">
          <span>动作按钮</span>
          <button
            class="dv-btn dv-btn-sm"
            :disabled="!wan.online.value"
            title="刷新动作列表"
            @click="wan.loadActions()"
          >
            <RefreshCw :size="12" />
          </button>
        </div>

        <div class="run-status">
          <span class="status-label">{{ runningName ? `执行中：${runningName}` : '动作空闲' }}</span>
          <button
            v-if="runningName"
            class="dv-btn dv-btn-danger dv-btn-sm"
            :disabled="!wan.controlling.value"
            @click="cancel"
          >
            <Square :size="12" /> 取消
          </button>
        </div>

        <div class="action-list dv-scroll">
          <div v-if="!activeActions.length" class="empty-tip">
            {{ wan.online.value ? '暂无预设或序列，请在配置后台添加' : '连接车辆后加载动作按钮' }}
          </div>
          <button
            v-for="item in activeActions"
            :key="item.action.name"
            class="dv-btn action-btn"
            :class="{ running: runningName === item.action.name }"
            :disabled="!canRun"
            @click="triggerAction(item.action.name)"
          >
            <span class="action-name">{{ item.action.name }}</span>
            <span class="action-kind mono">
              {{ item.action.preset ? '预设' : item.action.sequence ? `${item.action.sequence.steps.length}步` : '动作' }}
              <Play :size="12" />
            </span>
          </button>
        </div>
      </div>
    </dv-border-box-13>
  </div>
</template>

<style scoped>
.action-rail {
  width: 100%;
  height: 100%;
  min-height: 0;
}

.rail-inner {
  display: flex;
  flex-direction: column;
  gap: 8px;
  height: 100%;
  padding: 14px 12px 12px;
}

.run-status {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 6px 8px;
  border-radius: 3px;
  border: 1px solid rgba(0, 216, 255, 0.24);
  background: rgba(5, 22, 52, 0.72);
  font-size: 12px;
}

.status-label {
  color: var(--cyan-bright);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.action-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  flex: 1;
  min-height: 0;
  padding-right: 2px;
}

.action-btn {
  width: 100%;
  min-height: 42px;
  justify-content: space-between;
  padding: 8px 10px;
  text-align: left;
}

.action-btn.running {
  border-color: var(--ok);
  box-shadow: 0 0 12px rgba(24, 230, 154, 0.45);
}

.action-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.action-kind {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--cyan-bright);
  flex-shrink: 0;
}

.empty-tip {
  padding: 24px 8px;
  text-align: center;
  color: var(--text-dim);
  font-size: 12px;
  line-height: 1.5;
}
</style>
