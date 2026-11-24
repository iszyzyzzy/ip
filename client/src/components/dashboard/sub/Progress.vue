<script setup lang="ts">
import { computed } from 'vue'

interface Props {
  value?: number
  max?: number
  color?: string
  trackColor?: string
  height?: number
  animated?: boolean
  label?: string
}

const props = withDefaults(defineProps<Props>(), {
  value: 0,
  max: 100,
  color: '#18e69a',
  trackColor: 'rgba(0, 216, 255, 0.12)',
  height: 8,
  animated: true,
  label: '',
})

const percentage = computed(() => {
  const max = props.max > 0 ? props.max : 100
  return Math.max(0, Math.min(100, (props.value / max) * 100))
})

const barStyle = computed(() => ({
  '--progress': `${percentage.value}%`,
  '--progress-color': props.color,
  '--track-color': props.trackColor,
  '--progress-height': `${props.height}px`,
}))
</script>

<template>
  <div
    class="capsule-progress"
    :class="{ 'is-animated': animated }"
    :style="barStyle"
    role="progressbar"
    :aria-label="label || 'Progress'"
    :aria-valuemin="0"
    :aria-valuemax="max"
    :aria-valuenow="Math.max(0, Math.min(value, max))"
  >
    <div class="capsule-progress__fill" />
  </div>
</template>

<style scoped>
.capsule-progress {
  width: 100%;
  height: var(--progress-height);
  overflow: hidden;
  border-radius: 999px;
  background: var(--track-color);
  box-shadow:
    inset 0 0 0 1px rgba(0, 216, 255, 0.08),
    0 0 10px rgba(0, 216, 255, 0.08);
}

.capsule-progress__fill {
  width: var(--progress);
  height: 100%;
  border-radius: inherit;
  background: var(--progress-color);
  box-shadow: 0 0 10px color-mix(in srgb, var(--progress-color) 70%, transparent);
}

.is-animated .capsule-progress__fill {
  transition: width 280ms cubic-bezier(0.22, 1, 0.36, 1);
}
</style>