<script setup lang="ts">
import { ref } from 'vue'
const props = defineProps<{ x: number, y: number }>()
const emit = defineEmits<{ change: [x: number, y: number] }>()
const pad = ref<HTMLElement>()
let pointer: number | null = null
const clamp = (v: number) => Math.max(-1, Math.min(1, v))
function update(event: PointerEvent) {
  const r = pad.value!.getBoundingClientRect()
  emit('change', Math.round(clamp(2 * (event.clientX - r.left) / r.width - 1) * 100) / 100,
    Math.round(clamp(2 * (event.clientY - r.top) / r.height - 1) * 100) / 100)
}
function down(event: PointerEvent) {
  if (pointer !== null) return
  pointer = event.pointerId
  pad.value!.setPointerCapture(pointer)
  update(event)
}
function move(event: PointerEvent) { if (event.pointerId === pointer) update(event) }
function end(event: PointerEvent) { if (event.pointerId === pointer) pointer = null }
function key(event: KeyboardEvent) {
  const offset: Record<string, [number, number]> = { ArrowLeft: [-0.05, 0], ArrowRight: [0.05, 0], ArrowUp: [0, -0.05], ArrowDown: [0, 0.05] }
  if (event.key === 'Home') { event.preventDefault(); emit('change', 0, 0); return }
  const d = offset[event.key]
  if (d) { event.preventDefault(); emit('change', clamp(props.x + d[0]), clamp(props.y + d[1])) }
}
</script>
<template>
  <div ref="pad" role="application" aria-label="注视位置，点击或拖动选择，也可用方向键微调和Home回中" tabindex="0"
    class="relative h-48 w-full touch-none overflow-hidden rounded-xl border bg-muted/40 outline-none focus-visible:ring-2 focus-visible:ring-ring"
    @pointerdown="down" @pointermove="move" @pointerup="end" @pointercancel="end" @lostpointercapture="end" @keydown="key">
    <div class="pointer-events-none absolute inset-x-0 top-1/2 h-px bg-border" />
    <div class="pointer-events-none absolute inset-y-0 left-1/2 w-px bg-border" />
    <span class="pointer-events-none absolute inset-x-0 top-2 text-center text-xs text-muted-foreground">向上看</span>
    <span class="pointer-events-none absolute bottom-2 inset-x-0 text-center text-xs text-muted-foreground">向下看</span>
    <span class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">左</span>
    <span class="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-xs text-muted-foreground">右</span>
    <div class="pointer-events-none absolute size-5 -translate-x-1/2 -translate-y-1/2 rounded-full border-4 border-background bg-primary shadow ring-1 ring-primary/40"
      :style="{ left: `${50 + x * 44}%`, top: `${50 + y * 40}%` }" />
  </div>
</template>
