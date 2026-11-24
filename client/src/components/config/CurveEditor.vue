<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue'
import { Play, Plus, Square, Trash2 } from 'lucide-vue-next'
import {
  CURVE_TEMPLATES,
  clamp,
  cloneKeyframes,
  evalCustomCurve,
  evalCustomVelocity,
  evalCurveWeight,
  validateCustomCurve,
} from '@/lib/curve-math'
import type { CurveKeyframe, CustomCurve, MotionCurveType } from '@/lib/types'

const props = withDefaults(defineProps<{
  modelCurve?: MotionCurveType
  modelCustom?: CustomCurve
  durationMs?: number
  allowInherit?: boolean
  isInherited?: boolean
  inheritedLabel?: string
}>(), {
  modelCurve: 'SERVO_NATIVE',
  durationMs: 1000,
  allowInherit: false,
  isInherited: false,
  inheritedLabel: '继承序列默认',
})

const emit = defineEmits<{
  'update:modelCurve': [curve: MotionCurveType]
  'update:modelCustom': [custom: CustomCurve | undefined]
  'clearOverride': []
}>()

const PAD = { left: 42, right: 20, top: 18, bottom: 28 }
const VB_W = 640
const VB_H = 270
const PLOT_W = VB_W - PAD.left - PAD.right
const PLOT_H = VB_H - PAD.top - PAD.bottom

const svgRef = ref<SVGSVGElement | null>(null)
const selectedIdx = ref(0)
const showSamples = ref(true)
const showVelocity = ref(true)
const mirrorHandles = ref(true)
const dragState = ref<{
  kind: 'point' | 'in' | 'out'
  index: number
  lastT: number
  lastV: number
} | null>(null)

const previewT = ref<number | null>(null)
let previewRaf = 0

function stopPreview() {
  cancelAnimationFrame(previewRaf)
  previewRaf = 0
  previewT.value = null
}
onUnmounted(stopPreview)

function togglePreview() {
  if (previewT.value != null) {
    stopPreview()
    return
  }
  const dur = Math.max(200, props.durationMs || 1000)
  const start = performance.now()
  const tick = (now: number) => {
    const p = (now - start) / dur
    if (p >= 1) {
      previewT.value = 1
      window.setTimeout(() => {
        if (previewT.value === 1) stopPreview()
      }, 260)
      return
    }
    previewT.value = p
    previewRaf = requestAnimationFrame(tick)
  }
  previewRaf = requestAnimationFrame(tick)
}

const keyframes = computed<CurveKeyframe[]>(() => {
  const raw = props.modelCustom?.keyframes
  if (raw && raw.length >= 2) return raw
  return CURVE_TEMPLATES[1]!.keyframes
})

watch(() => keyframes.value.length, len => {
  if (selectedIdx.value >= len) selectedIdx.value = len - 1
})

function toX(t: number) {
  return PAD.left + clamp(t, 0, 1) * PLOT_W
}
function toY(v: number) {
  return PAD.top + (1 - clamp(v, 0, 1)) * PLOT_H
}
function clientToNorm(e: PointerEvent): { t: number, v: number } {
  const el = svgRef.value
  if (!el) return { t: 0, v: 0 }
  const rect = el.getBoundingClientRect()
  const sx = ((e.clientX - rect.left) / rect.width) * VB_W
  const sy = ((e.clientY - rect.top) / rect.height) * VB_H
  return {
    t: clamp((sx - PAD.left) / PLOT_W, 0, 1),
    v: clamp(1 - (sy - PAD.top) / PLOT_H, 0, 1),
  }
}

const activeCurveMode = computed<MotionCurveType>(() =>
  props.isInherited ? 'SERVO_NATIVE' : props.modelCurve,
)

const isCustom = computed(() => !props.isInherited && props.modelCurve === 'CUSTOM')

const curveValidationError = computed(() =>
  isCustom.value ? validateCustomCurve(keyframes.value) : null,
)

function selectMode(next: string) {
  if (next === 'INHERIT') {
    emit('clearOverride')
    return
  }
  const mode = next as MotionCurveType
  emit('update:modelCurve', mode)
  if (mode === 'CUSTOM' && (!props.modelCustom?.keyframes || props.modelCustom.keyframes.length < 2)) {
    emit('update:modelCustom', { keyframes: cloneKeyframes(CURVE_TEMPLATES[1]!.keyframes) })
  }
}

function applyTemplate(id: string) {
  const tpl = CURVE_TEMPLATES.find(item => item.id === id)
  if (!tpl) return
  emit('update:modelCurve', 'CUSTOM')
  emit('update:modelCustom', { keyframes: cloneKeyframes(tpl.keyframes) })
  selectedIdx.value = 0
}

function commitKeyframes(next: CurveKeyframe[]) {
  emit('update:modelCustom', { keyframes: next })
}

const mainPathD = computed(() => {
  if (isCustom.value) {
    const kfs = keyframes.value
    const parts: string[] = [`M ${toX(kfs[0]!.t).toFixed(1)} ${toY(kfs[0]!.v).toFixed(1)}`]
    for (let i = 0; i < kfs.length - 1; i++) {
      const a = kfs[i]!
      const b = kfs[i + 1]!
      parts.push(
        `C ${toX(a.out_t).toFixed(1)} ${toY(a.out_v).toFixed(1)}, ${toX(b.in_t).toFixed(1)} ${toY(b.in_v).toFixed(1)}, ${toX(b.t).toFixed(1)} ${toY(b.v).toFixed(1)}`,
      )
    }
    return parts.join(' ')
  }
  const mode = props.modelCurve
  const pts: string[] = []
  for (let i = 0; i <= 80; i++) {
    const t = i / 80
    const v = evalCurveWeight(mode, undefined, t)
    pts.push(`${i === 0 ? 'M' : 'L'} ${toX(t).toFixed(1)} ${toY(v).toFixed(1)}`)
  }
  return pts.join(' ')
})

const velocityPathD = computed(() => {
  if (!showVelocity.value || props.modelCurve === 'SERVO_NATIVE') return ''
  const raw: number[] = []
  const steps = 80
  for (let i = 0; i <= steps; i++) {
    const t = i / steps
    if (isCustom.value) {
      raw.push(evalCustomVelocity(keyframes.value, t))
    } else {
      const eps = 0.005
      const v0 = evalCurveWeight(props.modelCurve, undefined, Math.max(0, t - eps))
      const v1 = evalCurveWeight(props.modelCurve, undefined, Math.min(1, t + eps))
      raw.push((v1 - v0) / (Math.min(1, t + eps) - Math.max(0, t - eps)))
    }
  }
  const maxVel = Math.max(1.5, ...raw)
  return raw
    .map((vel, i) => `${i === 0 ? 'M' : 'L'} ${toX(i / steps).toFixed(1)} ${toY(vel / maxVel).toFixed(1)}`)
    .join(' ')
})

const sampleDots = computed(() => {
  if (!showSamples.value || props.modelCurve === 'SERVO_NATIVE') return []
  const dur = Math.max(80, props.durationMs || 1000)
  const dots: { x: number, y: number, t: number }[] = []
  for (let ms = 40; ms < dur; ms += 40) {
    const t = ms / dur
    const v = evalCurveWeight(props.modelCurve, keyframes.value, t)
    dots.push({ x: toX(t), y: toY(v), t })
  }
  dots.push({ x: toX(1), y: toY(1), t: 1 })
  return dots
})

const previewWeight = computed(() => {
  if (previewT.value == null) return 0
  return evalCurveWeight(props.modelCurve, keyframes.value, previewT.value)
})

function startDrag(kind: 'point' | 'in' | 'out', index: number, e: PointerEvent) {
  if (!isCustom.value) return
  e.stopPropagation()
  selectedIdx.value = index
  const k = keyframes.value[index]!
  dragState.value = { kind, index, lastT: k.t, lastV: k.v }
  ;(e.target as Element | null)?.setPointerCapture?.(e.pointerId)
}

function onPointerMove(e: PointerEvent) {
  const drag = dragState.value
  if (!drag || !isCustom.value) return
  const next = cloneKeyframes(keyframes.value)
  const i = drag.index
  const curr = next[i]!
  const prev = i > 0 ? next[i - 1]! : null
  const after = i < next.length - 1 ? next[i + 1]! : null
  const { t, v } = clientToNorm(e)

  if (drag.kind === 'point') {
    if (i === 0 || i === next.length - 1) return
    const minT = prev!.t + 0.015
    const maxT = after!.t - 0.015
    if (minT > maxT) return
    const newT = clamp(Math.round(t * 1000) / 1000, minT, maxT)
    const newV = clamp(Math.round(v * 1000) / 1000, 0, 1)
    const dt = newT - curr.t
    const dv = newV - curr.v
    curr.t = newT
    curr.v = newV
    curr.in_t = clamp(curr.in_t + dt, prev!.t, curr.t)
    curr.in_v = clamp(curr.in_v + dv, 0, 1)
    curr.out_t = clamp(curr.out_t + dt, curr.t, after!.t)
    curr.out_v = clamp(curr.out_v + dv, 0, 1)
    // 同时保证相邻点的出/入手柄不越过移动后的当前帧时间。
    prev!.out_t = clamp(prev!.out_t, prev!.t, curr.t)
    after!.in_t = clamp(after!.in_t, curr.t, after!.t)
  } else if (drag.kind === 'in' && prev) {
    curr.in_t = clamp(Math.round(t * 1000) / 1000, prev.t, curr.t)
    curr.in_v = clamp(Math.round(v * 1000) / 1000, 0, 1)
    if (mirrorHandles.value && after) {
      const dx = curr.t - curr.in_t
      const dy = curr.v - curr.in_v
      const lenIn = Math.hypot(dx, dy)
      if (lenIn > 1e-4) {
        const lenOut = Math.max(0.05, Math.hypot(curr.out_t - curr.t, curr.out_v - curr.v))
        const scale = lenOut / lenIn
        curr.out_t = clamp(curr.t + dx * scale, curr.t, after.t)
        curr.out_v = clamp(curr.v + dy * scale, 0, 1)
      }
    }
  } else if (drag.kind === 'out' && after) {
    curr.out_t = clamp(Math.round(t * 1000) / 1000, curr.t, after.t)
    curr.out_v = clamp(Math.round(v * 1000) / 1000, 0, 1)
    if (mirrorHandles.value && prev) {
      const dx = curr.out_t - curr.t
      const dy = curr.out_v - curr.v
      const lenOut = Math.hypot(dx, dy)
      if (lenOut > 1e-4) {
        const lenIn = Math.max(0.05, Math.hypot(curr.t - curr.in_t, curr.v - curr.in_v))
        const scale = lenIn / lenOut
        curr.in_t = clamp(curr.t - dx * scale, prev.t, curr.t)
        curr.in_v = clamp(curr.v - dy * scale, 0, 1)
      }
    }
  }

  commitKeyframes(next)
}

function endDrag() {
  dragState.value = null
}

function addKeyframeAt(tRaw?: number) {
  if (!isCustom.value) {
    selectMode('CUSTOM')
  }
  const next = cloneKeyframes(keyframes.value)
  if (next.length >= 16) return

  let t = tRaw
  if (t == null) {
    // 默认在当前选中关键帧与其下一帧（或前一帧）中点插入。
    const i = Math.min(selectedIdx.value, next.length - 2)
    t = 0.5 * (next[i]!.t + next[i + 1]!.t)
  }
  t = clamp(Math.round(t * 1000) / 1000, 0.02, 0.98)

  const idx = next.findIndex(k => Math.abs(k.t - t!) < 0.015)
  if (idx !== -1) {
    selectedIdx.value = idx
    return
  }

  const insertAt = next.findIndex(k => k.t > t!)
  const a = next[insertAt - 1]!
  const b = next[insertAt]!
  const v = Math.round(evalCustomCurve(next, t) * 1000) / 1000
  const spanLeft = (t - a.t) * 0.35
  const spanRight = (b.t - t) * 0.35
  const kf: CurveKeyframe = {
    t,
    v,
    in_t: clamp(Math.round((t - spanLeft) * 1000) / 1000, a.t, t),
    in_v: v,
    out_t: clamp(Math.round((t + spanRight) * 1000) / 1000, t, b.t),
    out_v: v,
  }
  a.out_t = clamp(a.out_t, a.t, t)
  b.in_t = clamp(b.in_t, t, b.t)
  next.splice(insertAt, 0, kf)
  selectedIdx.value = insertAt
  commitKeyframes(next)
}

function onCanvasDblClick(e: MouseEvent) {
  if (!isCustom.value || !svgRef.value) return
  const rect = svgRef.value.getBoundingClientRect()
  const sx = ((e.clientX - rect.left) / rect.width) * VB_W
  const t = clamp((sx - PAD.left) / PLOT_W, 0.02, 0.98)
  addKeyframeAt(t)
}

function removeSelected() {
  if (!isCustom.value) return
  const i = selectedIdx.value
  if (i <= 0 || i >= keyframes.value.length - 1) return
  const next = cloneKeyframes(keyframes.value)
  next.splice(i, 1)
  selectedIdx.value = Math.max(0, i - 1)
  commitKeyframes(next)
}

function updateSelectedField(field: keyof CurveKeyframe, raw: number) {
  if (!isCustom.value || !Number.isFinite(raw)) return
  const i = selectedIdx.value
  const next = cloneKeyframes(keyframes.value)
  const curr = next[i]!
  const prev = i > 0 ? next[i - 1]! : null
  const after = i < next.length - 1 ? next[i + 1]! : null

  if (field === 't' && prev && after) {
    curr.t = clamp(raw, prev.t + 0.015, after.t - 0.015)
    curr.in_t = clamp(curr.in_t, prev.t, curr.t)
    curr.out_t = clamp(curr.out_t, curr.t, after.t)
  } else if (field === 'v' && prev && after) {
    curr.v = clamp(raw, 0, 1)
  } else if (field === 'in_t' && prev) {
    curr.in_t = clamp(raw, prev.t, curr.t)
  } else if (field === 'in_v' && prev) {
    curr.in_v = clamp(raw, 0, 1)
  } else if (field === 'out_t' && after) {
    curr.out_t = clamp(raw, curr.t, after.t)
  } else if (field === 'out_v' && after) {
    curr.out_v = clamp(raw, 0, 1)
  }
  commitKeyframes(next)
}

const selectedKf = computed(() => keyframes.value[selectedIdx.value] ?? keyframes.value[0]!)
</script>

<template>
  <div class="curve-editor dv-card">
    <div class="editor-toolbar">
      <div class="mode-tabs">
        <button
          v-if="allowInherit"
          type="button"
          class="dv-btn dv-btn-sm"
          :class="{ active: isInherited }"
          @click="selectMode('INHERIT')"
        >
          {{ inheritedLabel }}
        </button>
        <button
          v-for="item in [
            { value: 'SERVO_NATIVE', label: '舵机原生' },
            { value: 'S_CURVE', label: '三次 S 曲线' },
            { value: 'S_CURVE_STRONG', label: '强 S 曲线' },
            { value: 'CUSTOM', label: 'AE 自定义曲线' },
          ]"
          :key="item.value"
          type="button"
          class="dv-btn dv-btn-sm"
          :class="{ active: !isInherited && activeCurveMode === item.value }"
          @click="selectMode(item.value)"
        >
          {{ item.label }}
        </button>
      </div>

      <div class="tool-right">
        <select
          v-if="isCustom"
          class="dv-select tpl-select"
          @change="applyTemplate(($event.target as HTMLSelectElement).value); ($event.target as HTMLSelectElement).value = ''"
        >
          <option value="">载入 AE 缓动模板…</option>
          <option v-for="t in CURVE_TEMPLATES" :key="t.id" :value="t.id">{{ t.label }}</option>
        </select>

        <button type="button" class="dv-btn dv-btn-sm" :class="{ active: showSamples }" @click="showSamples = !showSamples">
          40ms采样
        </button>
        <button type="button" class="dv-btn dv-btn-sm" :class="{ active: showVelocity }" @click="showVelocity = !showVelocity">
          速度线
        </button>
        <button
          v-if="isCustom"
          type="button"
          class="dv-btn dv-btn-sm"
          :class="{ active: mirrorHandles }"
          title="拖动一侧手柄时保持切线共线"
          @click="mirrorHandles = !mirrorHandles"
        >
          切线联动
        </button>
        <button type="button" class="dv-btn dv-btn-sm dv-btn-ok" @click="togglePreview">
          <component :is="previewT != null ? Square : Play" :size="12" />
          {{ previewT != null ? '停止' : '预览' }}
        </button>
      </div>
    </div>

    <div class="canvas-wrap">
      <svg
        ref="svgRef"
        :viewBox="`0 0 ${VB_W} ${VB_H}`"
        class="curve-svg"
        @dblclick="onCanvasDblClick"
        @pointermove="onPointerMove"
        @pointerup="endDrag"
        @pointercancel="endDrag"
      >
        <!-- 背景坐标网格 -->
        <g class="grid-lines">
          <line
            v-for="i in 11"
            :key="'vx' + i"
            :x1="toX((i - 1) / 10)"
            :y1="PAD.top"
            :x2="toX((i - 1) / 10)"
            :y2="PAD.top + PLOT_H"
            :class="{ major: (i - 1) % 5 === 0 }"
          />
          <line
            v-for="i in 6"
            :key="'hy' + i"
            :x1="PAD.left"
            :y1="toY((i - 1) / 5)"
            :x2="PAD.left + PLOT_W"
            :y2="toY((i - 1) / 5)"
            :class="{ major: i === 1 || i === 6 }"
          />
        </g>

        <!-- 轴刻度文字 -->
        <g class="axis-labels mono">
          <text v-for="p in [0, 0.25, 0.5, 0.75, 1]" :key="'tx' + p" :x="toX(p)" :y="VB_H - 8" text-anchor="middle">
            {{ Math.round(p * 100) }}%
          </text>
          <text v-for="p in [0, 0.5, 1]" :key="'ty' + p" :x="PAD.left - 8" :y="toY(p) + 4" text-anchor="end">
            {{ Math.round(p * 100) }}%
          </text>
        </g>

        <!-- 线性对比虚线 -->
        <line
          :x1="toX(0)"
          :y1="toY(0)"
          :x2="toX(1)"
          :y2="toY(1)"
          stroke="rgba(124, 168, 201, 0.28)"
          stroke-dasharray="4 4"
        />

        <!-- 速度参考折线 -->
        <path
          v-if="velocityPathD"
          :d="velocityPathD"
          fill="none"
          stroke="rgba(255, 176, 32, 0.55)"
          stroke-width="1.5"
          stroke-dasharray="3 3"
        />

        <!-- 运动位置曲线 -->
        <path
          :d="mainPathD"
          fill="none"
          :stroke="props.modelCurve === 'SERVO_NATIVE' && !isInherited ? '#7ca8c9' : '#00eaff'"
          :stroke-dasharray="props.modelCurve === 'SERVO_NATIVE' && !isInherited ? '6 4' : undefined"
          stroke-width="2.6"
        />

        <!-- 40ms 舵机插值采样点 -->
        <g v-if="sampleDots.length">
          <circle
            v-for="(dot, idx) in sampleDots"
            :key="idx"
            :cx="dot.x"
            :cy="dot.y"
            r="2.6"
            fill="#18e69a"
            opacity="0.82"
          />
        </g>

        <!-- 选中关键帧的贝塞尔控制线与手柄（所有关键帧均绘制细控制线，当前选中帧高亮可拖） -->
        <g v-if="isCustom">
          <template v-for="(kf, idx) in keyframes" :key="idx">
            <line
              v-if="idx > 0"
              :x1="toX(kf.t)"
              :y1="toY(kf.v)"
              :x2="toX(kf.in_t)"
              :y2="toY(kf.in_v)"
              :stroke="selectedIdx === idx ? '#ffb020' : 'rgba(255,176,32,0.35)'"
              stroke-width="1.5"
            />
            <line
              v-if="idx < keyframes.length - 1"
              :x1="toX(kf.t)"
              :y1="toY(kf.v)"
              :x2="toX(kf.out_t)"
              :y2="toY(kf.out_v)"
              :stroke="selectedIdx === idx ? '#ffb020' : 'rgba(255,176,32,0.35)'"
              stroke-width="1.5"
            />

            <circle
              v-if="idx > 0 && selectedIdx === idx"
              :cx="toX(kf.in_t)"
              :cy="toY(kf.in_v)"
              r="5.5"
              class="handle-dot"
              @pointerdown="startDrag('in', idx, $event)"
            />
            <circle
              v-if="idx < keyframes.length - 1 && selectedIdx === idx"
              :cx="toX(kf.out_t)"
              :cy="toY(kf.out_v)"
              r="5.5"
              class="handle-dot"
              @pointerdown="startDrag('out', idx, $event)"
            />

            <!-- 关键帧菱形/圆点 -->
            <circle
              :cx="toX(kf.t)"
              :cy="toY(kf.v)"
              :r="selectedIdx === idx ? 7 : 5.5"
              class="kf-dot"
              :class="{ selected: selectedIdx === idx, locked: idx === 0 || idx === keyframes.length - 1 }"
              @pointerdown="startDrag('point', idx, $event)"
            />
          </template>
        </g>

        <!-- 实时播放预览指针与臂位置球 -->
        <g v-if="previewT != null">
          <line
            :x1="toX(previewT)"
            :y1="PAD.top"
            :x2="toX(previewT)"
            :y2="PAD.top + PLOT_H"
            stroke="#ff3b53"
            stroke-width="1.5"
          />
          <circle
            :cx="toX(previewT)"
            :cy="toY(previewWeight)"
            r="6"
            fill="#ff3b53"
            stroke="#ffffff"
            stroke-width="1.5"
          />
        </g>
      </svg>

      <!-- 右侧模拟关节进度条 -->
      <div class="preview-gauge">
        <span class="mono">{{ Math.round(previewWeight * 100) }}%</span>
        <div class="gauge-track">
          <div class="gauge-dot" :style="{ bottom: `${previewWeight * 100}%` }" />
        </div>
        <span class="gauge-label">模拟位移</span>
      </div>
    </div>

    <div v-if="curveValidationError" class="err-line">{{ curveValidationError }}</div>

    <!-- AE 关键帧坐标微调与增删栏 -->
    <div v-if="isCustom" class="kf-inspector">
      <div class="kf-actions">
        <span class="mono">
          关键帧 {{ selectedIdx + 1 }}/{{ keyframes.length }}
          <template v-if="selectedIdx === 0">（起点锁定）</template>
          <template v-else-if="selectedIdx === keyframes.length - 1">（终点锁定）</template>
        </span>
        <button type="button" class="dv-btn dv-btn-sm" :disabled="keyframes.length >= 16" @click="addKeyframeAt()">
          <Plus :size="12" /> 打点新增帧
        </button>
        <button
          type="button"
          class="dv-btn dv-btn-sm dv-btn-danger"
          :disabled="selectedIdx === 0 || selectedIdx === keyframes.length - 1"
          @click="removeSelected"
        >
          <Trash2 :size="12" /> 删除选中帧
        </button>
      </div>

      <div class="num-grid mono">
        <label>
          时间 t
          <input
            type="number"
            step="0.01"
            min="0"
            max="1"
            class="dv-input"
            :disabled="selectedIdx === 0 || selectedIdx === keyframes.length - 1"
            :value="selectedKf.t.toFixed(3)"
            @change="updateSelectedField('t', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
        <label>
          进度 v
          <input
            type="number"
            step="0.01"
            min="0"
            max="1"
            class="dv-input"
            :disabled="selectedIdx === 0 || selectedIdx === keyframes.length - 1"
            :value="selectedKf.v.toFixed(3)"
            @change="updateSelectedField('v', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
        <label v-if="selectedIdx > 0">
          入柄 t
          <input
            type="number"
            step="0.01"
            min="0"
            max="1"
            class="dv-input"
            :value="selectedKf.in_t.toFixed(3)"
            @change="updateSelectedField('in_t', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
        <label v-if="selectedIdx > 0">
          入柄 v
          <input
            type="number"
            step="0.01"
            min="0"
            max="1"
            class="dv-input"
            :value="selectedKf.in_v.toFixed(3)"
            @change="updateSelectedField('in_v', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
        <label v-if="selectedIdx < keyframes.length - 1">
          出柄 t
          <input
            type="number"
            step="0.01"
            min="0"
            max="1"
            class="dv-input"
            :value="selectedKf.out_t.toFixed(3)"
            @change="updateSelectedField('out_t', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
        <label v-if="selectedIdx < keyframes.length - 1">
          出柄 v
          <input
            type="number"
            step="0.01"
            min="0"
            max="1"
            class="dv-input"
            :value="selectedKf.out_v.toFixed(3)"
            @change="updateSelectedField('out_v', Number(($event.target as HTMLInputElement).value))"
          />
        </label>
      </div>
    </div>
  </div>
</template>

<style scoped>
.curve-editor {
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.editor-toolbar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.mode-tabs,
.tool-right {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}

.tpl-select {
  width: 190px;
  padding: 4px 8px;
  font-size: 12px;
}

.canvas-wrap {
  display: grid;
  grid-template-columns: 1fr 56px;
  gap: 10px;
  align-items: stretch;
}

.curve-svg {
  width: 100%;
  height: 245px;
  border: 1px solid rgba(0, 216, 255, 0.28);
  border-radius: 4px;
  background: rgba(2, 9, 24, 0.92);
  touch-action: none;
  cursor: crosshair;
}

.grid-lines line {
  stroke: rgba(0, 216, 255, 0.08);
  stroke-width: 1;
}

.grid-lines line.major {
  stroke: rgba(0, 216, 255, 0.24);
}

.axis-labels text {
  fill: var(--text-dim);
  font-size: 10px;
}

.kf-dot {
  fill: #00d8ff;
  stroke: #030a18;
  stroke-width: 2;
  cursor: grab;
}

.kf-dot.locked {
  fill: #18e69a;
  cursor: pointer;
}

.kf-dot.selected {
  fill: #ffffff;
  stroke: #00d8ff;
  stroke-width: 2.5;
}

.handle-dot {
  fill: #ffb020;
  stroke: #030a18;
  stroke-width: 1.5;
  cursor: grab;
}

.preview-gauge {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: space-between;
  padding: 6px 4px;
  border: 1px solid rgba(0, 216, 255, 0.22);
  border-radius: 4px;
  background: rgba(3, 14, 34, 0.78);
  font-size: 11px;
  color: var(--cyan-bright);
}

.gauge-track {
  position: relative;
  width: 10px;
  flex: 1;
  margin: 8px 0;
  border-radius: 5px;
  background: rgba(0, 216, 255, 0.14);
  border: 1px solid rgba(0, 216, 255, 0.35);
}

.gauge-dot {
  position: absolute;
  left: 50%;
  width: 16px;
  height: 16px;
  transform: translate(-50%, 50%);
  border-radius: 50%;
  background: var(--cyan-bright);
  box-shadow: 0 0 10px var(--cyan-bright);
}

.gauge-label {
  font-size: 10px;
  color: var(--text-dim);
}

.err-line {
  padding: 6px 10px;
  border: 1px solid rgba(255, 59, 83, 0.7);
  background: rgba(130, 12, 28, 0.45);
  color: #ffb8c0;
  font-size: 12px;
  border-radius: 3px;
}

.kf-inspector {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-top: 6px;
  border-top: 1px solid rgba(0, 216, 255, 0.2);
}

.kf-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  font-size: 12px;
}

.hint {
  margin-left: auto;
  color: var(--text-dim);
  font-size: 11px;
}

.num-grid {
  display: grid;
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: 8px;
}

.num-grid label {
  display: flex;
  flex-direction: column;
  gap: 3px;
  font-size: 11px;
  color: var(--text-dim);
}
</style>
