import type { CurveKeyframe, MotionCurveType } from './types'

export function clamp(val: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, val))
}

export function cubic(p0: number, p1: number, p2: number, p3: number, u: number): number {
  const inv = 1 - u
  return (
    inv * inv * inv * p0 +
    3 * inv * inv * u * p1 +
    3 * inv * u * u * p2 +
    u * u * u * p3
  )
}

function cubicDeriv(p0: number, p1: number, p2: number, p3: number, u: number): number {
  const inv = 1 - u
  return (
    3 * inv * inv * (p1 - p0) +
    6 * inv * u * (p2 - p1) +
    3 * u * u * (p3 - p2)
  )
}

export function cloneKeyframes(kfs: CurveKeyframe[]): CurveKeyframe[] {
  return kfs.map(k => ({ ...k }))
}

export function validateCustomCurve(kfs: CurveKeyframe[]): string | null {
  if (kfs.length < 2 || kfs.length > 16) return '关键帧数量须在 2～16 个之间'
  for (let i = 0; i < kfs.length; i++) {
    const k = kfs[i]!
    for (const v of [k.t, k.v, k.in_t, k.in_v, k.out_t, k.out_v]) {
      if (!Number.isFinite(v)) return `第 ${i + 1} 个关键帧存在无效数值`
      if (v < -1e-6 || v > 1 + 1e-6) return `第 ${i + 1} 个关键帧及手柄必须落在 [0, 1] 安全区内`
    }
  }
  const first = kfs[0]!
  const last = kfs[kfs.length - 1]!
  if (Math.abs(first.t) > 1e-6 || Math.abs(first.v) > 1e-6) return '起点必须固定在 (0, 0)'
  if (Math.abs(last.t - 1) > 1e-6 || Math.abs(last.v - 1) > 1e-6) return '终点必须固定在 (1, 1)'
  for (let i = 0; i < kfs.length - 1; i++) {
    const a = kfs[i]!
    const b = kfs[i + 1]!
    if (b.t - a.t < 0.01) return `第 ${i + 1} 与 ${i + 2} 帧时间间隔不能小于 0.01`
    if (a.out_t < a.t - 1e-6 || a.out_t > b.t + 1e-6) return `第 ${i + 1} 帧出点时间手柄须在相邻帧之间`
    if (b.in_t < a.t - 1e-6 || b.in_t > b.t + 1e-6) return `第 ${i + 2} 帧入点时间手柄须在相邻帧之间`
  }
  return null
}

/** 与后端 Rust ResolvedCurve 完全一致的 24 步二分求解。 */
function solveSegment(a: CurveKeyframe, b: CurveKeyframe, t: number): { u: number } {
  const x0 = a.t
  const x1 = clamp(a.out_t, a.t, b.t)
  const x2 = clamp(b.in_t, a.t, b.t)
  const x3 = b.t
  let lo = 0
  let hi = 1
  for (let i = 0; i < 24; i++) {
    const mid = 0.5 * (lo + hi)
    if (cubic(x0, x1, x2, x3, mid) < t) lo = mid
    else hi = mid
  }
  return { u: 0.5 * (lo + hi) }
}

export function evalCustomCurve(kfs: CurveKeyframe[], tRaw: number): number {
  const t = clamp(tRaw, 0, 1)
  if (kfs.length < 2) return t
  if (t <= 0) return 0
  if (t >= 1) return 1
  let idx = kfs.length - 2
  for (let i = 0; i < kfs.length - 1; i++) {
    if (t <= kfs[i + 1]!.t) {
      idx = i
      break
    }
  }
  const a = kfs[idx]!
  const b = kfs[idx + 1]!
  const { u } = solveSegment(a, b, t)
  return clamp(cubic(a.v, a.out_v, b.in_v, b.v, u), 0, 1)
}

export function evalCustomVelocity(kfs: CurveKeyframe[], tRaw: number): number {
  const t = clamp(tRaw, 0, 1)
  if (kfs.length < 2) return 1
  let idx = kfs.length - 2
  for (let i = 0; i < kfs.length - 1; i++) {
    if (t <= kfs[i + 1]!.t) {
      idx = i
      break
    }
  }
  const a = kfs[idx]!
  const b = kfs[idx + 1]!
  const x0 = a.t
  const x1 = clamp(a.out_t, a.t, b.t)
  const x2 = clamp(b.in_t, a.t, b.t)
  const x3 = b.t
  const { u } = solveSegment(a, b, t)
  const dxdt = cubicDeriv(x0, x1, x2, x3, u)
  const dydt = cubicDeriv(a.v, a.out_v, b.in_v, b.v, u)
  if (dxdt < 1e-4) return 0
  return Math.max(0, dydt / dxdt)
}

export function evalCurveWeight(
  mode: MotionCurveType | undefined,
  keyframes: CurveKeyframe[] | undefined,
  tRaw: number,
): number {
  const t = clamp(tRaw, 0, 1)
  switch (mode) {
    case 'S_CURVE':
      return t * t * (3 - 2 * t)
    case 'S_CURVE_STRONG':
      return t ** 4 * (35 + t * (-84 + t * (70 - 20 * t)))
    case 'CUSTOM':
      return keyframes && keyframes.length >= 2 ? evalCustomCurve(keyframes, t) : t
    default:
      return t
  }
}

export const CURVE_TEMPLATES: { id: string, label: string, keyframes: CurveKeyframe[] }[] = [
  {
    id: 'linear',
    label: '线性匀速 (Linear)',
    keyframes: [
      { t: 0, v: 0, in_t: 0, in_v: 0, out_t: 0.25, out_v: 0.25 },
      { t: 1, v: 1, in_t: 0.75, in_v: 0.75, out_t: 1, out_v: 1 },
    ],
  },
  {
    id: 'easeInOut',
    label: '标准缓入缓出 (Ease In-Out)',
    keyframes: [
      { t: 0, v: 0, in_t: 0, in_v: 0, out_t: 0.42, out_v: 0 },
      { t: 1, v: 1, in_t: 0.58, in_v: 1, out_t: 1, out_v: 1 },
    ],
  },
  {
    id: 'strongSCurve',
    label: '强力柔顺起步收尾 (AE 75/75)',
    keyframes: [
      { t: 0, v: 0, in_t: 0, in_v: 0, out_t: 0.75, out_v: 0 },
      { t: 1, v: 1, in_t: 0.25, in_v: 1, out_t: 1, out_v: 1 },
    ],
  },
  {
    id: 'easeOut',
    label: '急启缓停 (Ease Out)',
    keyframes: [
      { t: 0, v: 0, in_t: 0, in_v: 0, out_t: 0.16, out_v: 0.8 },
      { t: 1, v: 1, in_t: 0.35, in_v: 1, out_t: 1, out_v: 1 },
    ],
  },
  {
    id: 'easeIn',
    label: '蓄力缓启 (Ease In)',
    keyframes: [
      { t: 0, v: 0, in_t: 0, in_v: 0, out_t: 0.65, out_v: 0 },
      { t: 1, v: 1, in_t: 0.85, in_v: 0.2, out_t: 1, out_v: 1 },
    ],
  },
  {
    id: 'midHold',
    label: '两段式·中段微停 (3点)',
    keyframes: [
      { t: 0, v: 0, in_t: 0, in_v: 0, out_t: 0.16, out_v: 0 },
      { t: 0.5, v: 0.52, in_t: 0.3, in_v: 0.52, out_t: 0.72, out_v: 0.52 },
      { t: 1, v: 1, in_t: 0.84, in_v: 1, out_t: 1, out_v: 1 },
    ],
  },
]
