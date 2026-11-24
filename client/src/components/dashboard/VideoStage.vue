<script setup lang="ts">
import { computed, ref, watchEffect } from 'vue'
import { Maximize2, Minimize2, RefreshCw } from 'lucide-vue-next'
import { clientConfig } from '@/lib/settings'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()

const videoEl = ref<HTMLVideoElement | null>(null)
const audioEl = ref<HTMLAudioElement | null>(null)
const coverMode = ref(false)

watchEffect(() => {
  if (videoEl.value) {
    videoEl.value.srcObject = wan.remoteVideoStream.value
  }
})

watchEffect(() => {
  const el = audioEl.value
  if (!el) return
  el.srcObject = wan.remoteAudioStream.value
  el.muted = !clientConfig.mic_listen_enabled
  // 开启自动半双工时，按住讲话自动压低车端回传监听音量以防回声。
  el.volume = clientConfig.auto_half_duplex && wan.talking.value ? 0.12 : 1.0
})

const hasVideo = computed(() => !!wan.remoteVideoStream.value)

const videoDisabled = computed(() => {
  if (!wan.online.value) return false
  if (!wan.capabilities.value.video) return true
  const phase = wan.snapshot.value?.video
  return phase === 'inactive' || phase === 'offline'
})

const resolutionText = computed(() => {
  const s = wan.rtcStats.value
  if (!s.videoWidth || !s.videoHeight) return '—'
  return `${s.videoWidth}×${s.videoHeight}`
})

const talkText = computed(() => {
  const t = wan.snapshot.value?.talk
  if (!t || t.phase === 'idle') return '未喊话'
  if (t.phase === 'accepted') return '喊话通道开启'
  if (t.phase === 'completed') return '喊话结束'
  if (t.phase === 'failed') return `喊话失败：${t.message || t.code}`
  return `喊话：${t.phase}`
})
</script>

<template>
  <div class="video-stage">
    <dv-border-box-11 title="实时图传监控" :title-width="220" :color="['#00d8ff', '#1d6fff']">
      <div class="stage-inner">
        <video
          v-show="hasVideo"
          ref="videoEl"
          autoplay
          playsinline
          muted
          class="video-feed"
          :class="{ cover: coverMode }"
        />
        <audio ref="audioEl" autoplay playsinline />

        <div v-if="!hasVideo" class="empty-stage">
          <dv-loading>{{ videoDisabled ? '车端未启用图传' : '正在等待图传信号…' }}</dv-loading>
        </div>

        <!-- 左上角视频 HUD：平时仅展示分辨率+FPS，仅未启用图传时提示 VIDEO -->
        <div class="hud-top-left mono">
          <span v-if="videoDisabled" class="dv-pill warn">
            VIDEO: 未启用
          </span>
          <span class="dv-pill">
            {{ resolutionText }} · {{ wan.rtcStats.value.videoFps ?? 0 }} FPS
          </span>
        </div>

        <!-- 右上角控制按钮 -->
        <div class="hud-top-right">
          <button
            class="dv-btn dv-btn-sm"
            :title="coverMode ? '切换为完整适应画面' : '切换为铺满裁剪'"
            @click="coverMode = !coverMode"
          >
            <component :is="coverMode ? Minimize2 : Maximize2" :size="14" />
          </button>
          <button
            v-if="wan.online.value"
            class="dv-btn dv-btn-sm"
            :disabled="wan.busy.value"
            title="重新建链"
            @click="wan.connect()"
          >
            <RefreshCw :size="14" />
          </button>
        </div>

        <!-- 画面冻结警告 -->
        <div v-if="wan.videoFrozen.value" class="freeze-banner">
          图传画面已冻结（超过 2 秒未收到新帧）
        </div>

        <!-- 右下角喊话状态提示 -->
        <div v-if="wan.online.value" class="hud-bottom-right mono">
          <span class="dv-pill" :class="wan.talking.value ? 'ok' : ''">
            {{ talkText }}
          </span>
        </div>
      </div>
    </dv-border-box-11>
  </div>
</template>

<style scoped>
.video-stage {
  width: 100%;
  height: 100%;
  min-height: 0;
}

.stage-inner {
  position: relative;
  width: 100%;
  height: 100%;
  padding: 46px 14px 14px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.video-feed {
  width: 100%;
  height: 100%;
  border-radius: 3px;
  border: 1px solid rgba(0, 216, 255, 0.35);
  background: #020814;
  object-fit: contain;
}

.video-feed.cover {
  object-fit: cover;
}

.empty-stage {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  border: 1px dashed rgba(0, 216, 255, 0.28);
  border-radius: 4px;
  background: radial-gradient(circle at center, rgba(10, 42, 92, 0.35), rgba(3, 10, 24, 0.92));
}

.hud-top-left {
  position: absolute;
  top: 56px;
  left: 24px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  pointer-events: none;
}

.hud-top-right {
  position: absolute;
  top: 56px;
  right: 24px;
  display: flex;
  gap: 6px;
}

.hud-bottom-right {
  position: absolute;
  bottom: 24px;
  right: 24px;
  pointer-events: none;
}

.freeze-banner {
  position: absolute;
  bottom: 26px;
  left: 50%;
  transform: translateX(-50%);
  padding: 6px 14px;
  border: 1px solid rgba(255, 176, 32, 0.85);
  background: rgba(90, 46, 0, 0.88);
  color: #ffe29e;
  font-size: 13px;
  font-weight: 700;
  border-radius: 3px;
}
</style>
