<script setup lang="ts">
import { computed } from 'vue'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()

const neck = computed(() => {
  const n = wan.state.value?.devices?.neck
  const toDeg = (rad: unknown) =>
    typeof rad === 'number' ? `${((rad * 180) / Math.PI).toFixed(1)}°` : '—'
  return {
    yaw: toDeg(n?.gm6020?.joint_position_rad),
    pitch: toDeg(n?.dm4310?.joint_position_rad),
  }
})

function armAngles(side: 'left_arm' | 'right_arm') {
  const arm = wan.state.value?.devices?.[side]
  if (!arm) return ['—', '—', '—', '—']
  return [0, 1, 2, 3].map(i => {
    const j = arm[String(i)]
    if (!j || j.error || typeof j.position_rad !== 'number') return '—'
    return `${((j.position_rad * 180) / Math.PI).toFixed(0)}°`
  })
}

const leftArm = computed(() => armAngles('left_arm'))
const rightArm = computed(() => armAngles('right_arm'))

const motion = computed(() => {
  const m = wan.state.value?.devices?.chassis?.motion
  return {
    speed: typeof m?.speed_m_s === 'number' ? m.speed_m_s.toFixed(2) : '0.00',
    steer: typeof m?.steering_raw_rad === 'number' ? ((m.steering_raw_rad * 180) / Math.PI).toFixed(1) : '0.0',
  }
})
</script>

<template>
  <div class="model-box">
    <div class="dv-section-title">
      <span>3D 姿态模型</span>
      <!-- <span class="dv-pill mono">{{ wan.state.value?.simulated ? 'SIM' : 'REAL' }}</span> -->
    </div>

    <dv-loading>Loading...</dv-loading>

    <!-- 3D 模型占位骨架区：保持标准数据视图结构，后续可直接替换为 Three.js Canvas -->
<!--     <div class="stage">
      <dv-decoration-12 class="radar" :scan-dur="6" :halo-dur="4" />
      <div class="wireframe-placeholder">
        <div class="wire-head">
          <div class="wire-eyes">
            <span /><span />
          </div>
        </div>
        <div class="wire-torso">
          <span class="wire-arm left" />
          <span class="wire-core" />
          <span class="wire-arm right" />
        </div>
        <div class="wire-base">
          <span /><span />
        </div>
        <div class="placeholder-tag">3D 机器人姿态模型占位</div>
      </div>
    </div>

    <div class="pose-grid mono">
      <div class="pose-cell">
        <span class="k">颈部 Y / P</span>
        <span class="v">{{ neck.yaw }} / {{ neck.pitch }}</span>
      </div>
      <div class="pose-cell">
        <span class="k">底盘 速 / 转</span>
        <span class="v">{{ motion.speed }}m/s · {{ motion.steer }}°</span>
      </div>
      <div class="pose-cell">
        <span class="k">左臂 J1~4</span>
        <span class="v">{{ leftArm.join(' ') }}</span>
      </div>
      <div class="pose-cell">
        <span class="k">右臂 J1~4</span>
        <span class="v">{{ rightArm.join(' ') }}</span>
      </div>
    </div> -->
  </div>
</template>

<style scoped>
.model-box {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.stage {
  position: relative;
  height: 196px;
  border: 1px solid rgba(0, 216, 255, 0.3);
  border-radius: 4px;
  background:
    radial-gradient(circle at center, rgba(0, 140, 255, 0.16), rgba(3, 10, 24, 0.92)),
    linear-gradient(rgba(0, 216, 255, 0.08) 1px, transparent 1px),
    linear-gradient(90deg, rgba(0, 216, 255, 0.08) 1px, transparent 1px);
  background-size: 100% 100%, 18px 18px, 18px 18px;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.radar {
  position: absolute;
  width: 150px;
  height: 150px;
  opacity: 0.48;
  pointer-events: none;
}

.wireframe-placeholder {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
}

.wire-head {
  width: 54px;
  height: 32px;
  border: 1.5px solid var(--cyan);
  border-radius: 6px;
  background: rgba(0, 216, 255, 0.08);
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 0 10px rgba(0, 216, 255, 0.25);
}

.wire-eyes {
  display: flex;
  gap: 12px;
}

.wire-eyes span {
  width: 9px;
  height: 12px;
  border-radius: 50%;
  background: var(--cyan-bright);
  box-shadow: 0 0 8px var(--cyan-bright);
}

.wire-torso {
  display: flex;
  align-items: flex-start;
  gap: 8px;
}

.wire-core {
  width: 50px;
  height: 44px;
  border: 1.5px solid rgba(0, 216, 255, 0.72);
  border-radius: 4px;
  background: rgba(0, 140, 255, 0.1);
}

.wire-arm {
  width: 10px;
  height: 38px;
  border: 1.5px dashed rgba(103, 243, 255, 0.7);
  border-radius: 3px;
}

.wire-base {
  display: flex;
  gap: 44px;
}

.wire-base span {
  width: 18px;
  height: 10px;
  border: 1.5px solid var(--cyan);
  border-radius: 3px;
  background: rgba(0, 216, 255, 0.16);
}

.placeholder-tag {
  margin-top: 2px;
  padding: 2px 8px;
  border: 1px solid rgba(0, 216, 255, 0.35);
  background: rgba(3, 14, 34, 0.88);
  color: var(--cyan-bright);
  font-size: 11px;
  letter-spacing: 0.06em;
}

.pose-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 4px;
}

.pose-cell {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 4px 8px;
  border: 1px solid rgba(0, 216, 255, 0.18);
  background: rgba(5, 20, 48, 0.65);
  border-radius: 3px;
  font-size: 11px;
}

.pose-cell .k {
  color: var(--text-dim);
}

.pose-cell .v {
  color: var(--cyan-bright);
}
</style>
