import { computed, reactive, ref, watch } from 'vue'
import { clientConfig } from './settings'
import type { ChassisMode } from './types'
import { useWanClient } from './wan-client'

// 与 robotd validation 一致的官方几何转向限制。
const OFFICIAL_STEER = { ACKERMANN: 0.698, CRAB: 1.571 } as const
const STEER_20_DEG = (20 * Math.PI) / 180
const SPEED_AT_WIDE_STEER = 0.525
const DRIVE_INTERVAL_MS = 50
const NECK_INTERVAL_MS = 50
const KEY_STEER_TRAVEL_MS = 400

const mode = ref<ChassisMode>('PARK')
const buttonVector = reactive({ x: 0, y: 0 })
const keyVector = reactive({ x: 0, y: 0 })
const padVector = reactive({ x: 0, y: 0 })
const neckStick = reactive({ x: 0, y: 0 })
const neckPose = reactive({ yaw: 0, pitch: 0 })
const activeGamepadName = ref('')

const chassisOutput = ref({ speed: 0, steer: 0, yaw: 0 })

const pressedKeys = new Set<string>()
let driveTimer = 0
let neckTimer = 0
let rafId = 0
let lastZeroSent = true
const keySteer = ref(0)
let steerTick = performance.now()
let padNeckDelta = { dx: 0, dy: 0 }
let lastNeckSend = 0
let neckTrailingTimer = 0
let listenersAttached = false
let stopFeedbackWatch: (() => void) | undefined

function clamp(v: number, lo: number, hi: number): number {
  return Math.max(lo, Math.min(hi, v))
}

function mapRange(v: number, [lo, hi]: readonly [number, number]): number {
  return v >= 0 ? v * hi : -v * lo
}

function getChassisLimits() {
  const wan = useWanClient()
  const c = wan.config.value?.chassis ?? {}
  return {
    speed: c.max_speed_m_s ?? 0.3,
    yaw: c.max_yaw_rate_rad_s ?? 0.5,
    steer: c.max_steering_rad ?? 0.698,
  }
}

function getNeckLimits() {
  const wan = useWanClient()
  const n = wan.config.value?.neck ?? {}
  return {
    yaw: [n.yaw?.min_rad ?? -0.5, n.yaw?.max_rad ?? 0.5] as const,
    pitch: [n.pitch?.min_rad ?? -0.5, n.pitch?.max_rad ?? 0.5] as const,
  }
}

function activeDriveAxis() {
  // 优先级：圆盘摇杆 > 键盘 > 外接手柄，避免多输入叠加。
  if (buttonVector.x !== 0 || buttonVector.y !== 0) return buttonVector
  if (clientConfig.keyboard_enabled && (keyVector.x !== 0 || keyVector.y !== 0)) {
    return { x: mode.value === 'SPIN' ? keyVector.x : keySteer.value, y: keyVector.y }
  }
  if (clientConfig.gamepad_enabled && (padVector.x !== 0 || padVector.y !== 0)) return padVector
  return { x: 0, y: 0 }
}

function computeChassisTarget() {
  const axis = activeDriveAxis()
  const lim = getChassisLimits()
  const gear = clientConfig.speed_gear ?? 'low'
  const gearSpeed = clamp(clientConfig.gear_speeds?.[gear] ?? 0.3, 0.01, lim.speed)
  const yawScale = gear === 'low' ? 0.35 : gear === 'mid' ? 0.65 : 1.0
  let speed = 0
  let steer = 0
  let yaw = 0

  if (mode.value === 'SPIN') {
    yaw = -axis.x * lim.yaw * yawScale
  } else if (mode.value !== 'PARK') {
    speed = axis.y * gearSpeed
    const maxSteer = Math.min(lim.steer, OFFICIAL_STEER[mode.value])
    steer = -axis.x * maxSteer
    if (Math.abs(steer) > STEER_20_DEG) {
      speed = clamp(speed, -SPEED_AT_WIDE_STEER, SPEED_AT_WIDE_STEER)
    }
  }
  const round = (v: number) => Math.round(v * 1000) / 1000
  const out = { speed: round(speed), steer: round(steer), yaw: round(yaw) }
  chassisOutput.value = out
  return out
}

function tickDrive() {
  const wan = useWanClient()
  const now = performance.now()
  const step = Math.min(DRIVE_INTERVAL_MS, now - steerTick) / KEY_STEER_TRAVEL_MS
  steerTick = now
  // 数字按键渐进打方向；纵向速度仍直接跟随 W/S，松开后立即停止。
  if (wan.controlling.value && clientConfig.keyboard_enabled && (keyVector.x || keyVector.y)) {
    keySteer.value += clamp(keyVector.x - keySteer.value, -step, step)
  } else {
    keySteer.value = 0
  }
  if (!wan.controlling.value) {
    lastZeroSent = true
    chassisOutput.value = { speed: 0, steer: 0, yaw: 0 }
    return
  }
  const out = computeChassisTarget()
  const isZero = out.speed === 0 && out.steer === 0 && out.yaw === 0
  if (isZero && lastZeroSent) return
  lastZeroSent = isZero
  wan.sendDrive({
    mode: mode.value,
    speed_m_s: out.speed,
    steering_rad: out.steer,
    yaw_rate_rad_s: out.yaw,
  })
}

const headStatus = computed(() => {
  const wan = useWanClient()
  const neck = wan.state.value?.devices?.neck
  if (!wan.online.value || !wan.config.value?.neck?.enabled || !neck?.gm6020 || !neck.dm4310) return '无反馈'
  const states = [neck.gm6020.state, neck.dm4310.state]
  if (states.includes('故障停止')) return '故障停止'
  if (states.includes('等待使能反馈')) return '等待使能'
  if (states.includes('停止')) return '已卸力'
  return states.every(s => s === '位置控制') ? '' : '无反馈'
})
const canCenterHead = computed(() => {
  const wan = useWanClient()
  return wan.controlling.value && !wan.state.value?.latched && !wan.state.value?.action &&
    headStatus.value !== '无反馈' && headStatus.value !== '等待使能'
})
const headDisabled = computed(() => !canCenterHead.value || !!headStatus.value)

function syncNeckFeedback() {
  const wan = useWanClient()
  const neck = wan.state.value?.devices?.neck
  const cfg = wan.config.value?.neck
  if (!cfg || !neck || (!headStatus.value && Date.now() - lastNeckSend < 400)) return
  for (const [axis, stick, key] of [['yaw','x','gm6020'], ['pitch','y','dm4310']] as const) {
    const motor = neck[key]
    const limits = cfg[axis]
    if (!limits || !motor) continue
    const angle = motor.state === '位置控制'
      ? (motor.target_rad - limits.offset_rad) / limits.direction : motor.joint_position_rad
    if (!Number.isFinite(angle)) continue
    neckPose[axis] = angle
    const span = angle >= 0 ? limits.max_rad : -limits.min_rad
    neckStick[stick] = span > 0 ? clamp(-angle / span, -1, 1) : 0
  }
}

function sendNeck(x: number, y: number, center: boolean, followEyes: boolean) {
  if (!canCenterHead.value || (!center && headDisabled.value)) return
  clearTimeout(neckTrailingTimer)
  const lim = getNeckLimits()
  neckStick.x = clamp(x,-1,1)
  neckStick.y = clamp(y,-1,1)
  neckPose.yaw = mapRange(-neckStick.x, lim.yaw)
  neckPose.pitch = mapRange(-neckStick.y, lim.pitch)
  const delay = NECK_INTERVAL_MS - (Date.now() - lastNeckSend)
  if (!center && delay > 0) {
    neckTrailingTimer = window.setTimeout(() => sendNeck(x,y,false,followEyes), delay)
    return
  }
  lastNeckSend = Date.now()
  const wan = useWanClient()
  const targets: Record<string, unknown> = { neck: { yaw_rad: neckPose.yaw, pitch_rad: neckPose.pitch } }
  if (followEyes && wan.config.value?.eyes?.enabled) {
    const eye = { gaze_x: x * 0.6, gaze_y: -y * 0.6, openness: 1, brow: 0 }
    targets.eyes = { left: eye, right: eye, roll: 0 }
  }
  wan.setTargets(targets).catch(err => { wan.errorMessage.value = String(err.message || err) })
}

function setNeckStick(x: number, y: number, followEyes = false) {
  sendNeck(x, y, false, followEyes)
}

function tickPadNeck() {
  if (padNeckDelta.dx === 0 && padNeckDelta.dy === 0) return
  setNeckStick(neckStick.x + padNeckDelta.dx * 0.06, neckStick.y + padNeckDelta.dy * 0.06)
}

function refreshKeyVector() {
  let x = 0
  let y = 0
  if (pressedKeys.has('KeyW') || pressedKeys.has('ArrowUp')) y += 1
  if (pressedKeys.has('KeyS') || pressedKeys.has('ArrowDown')) y -= 1
  if (pressedKeys.has('KeyA') || pressedKeys.has('ArrowLeft')) x -= 1
  if (pressedKeys.has('KeyD') || pressedKeys.has('ArrowRight')) x += 1
  if (pressedKeys.has('KeyQ')) x -= 1
  if (pressedKeys.has('KeyE')) x += 1
  keyVector.x = clamp(x, -1, 1)
  keyVector.y = clamp(y, -1, 1)
  computeChassisTarget()
}

function isEditableFocused(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null
  if (!el) return false
  const tag = el.tagName
  return tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || el.isContentEditable
}

function onKeyDown(e: KeyboardEvent) {
  if (isEditableFocused(e.target)) return
  if (e.code === 'Space') {
    const wan = useWanClient()
    if (wan.online.value) {
      e.preventDefault()
      stopAllInputs()
      wan.emergencyStop()
    }
    return
  }
  if (!clientConfig.keyboard_enabled) return
  if (e.code === 'Backquote') {
    e.preventDefault()
    if (!e.repeat) useWanClient().holdTalk('key')
    return
  }
  if (['KeyW', 'KeyA', 'KeyS', 'KeyD', 'KeyQ', 'KeyE', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'].includes(e.code)) {
    if (e.code.startsWith('Arrow')) e.preventDefault()
    pressedKeys.add(e.code)
    refreshKeyVector()
  }
}

function onKeyUp(e: KeyboardEvent) {
  if (e.code === 'Backquote') useWanClient().releaseTalk('key')
  if (pressedKeys.delete(e.code)) {
    refreshKeyVector()
  }
}

function pollGamepad() {
  if (clientConfig.gamepad_enabled && typeof navigator !== 'undefined' && navigator.getGamepads) {
    const pads = navigator.getGamepads()
    const pad = Array.from(pads).find(p => p && p.connected)
    if (pad) {
      activeGamepadName.value = pad.id
      const dz = (v: number) => (Math.abs(v) < 0.14 ? 0 : v)
      padVector.x = dz(pad.axes[0] ?? 0)
      padVector.y = -dz(pad.axes[1] ?? 0)
      const rx = dz(pad.axes[2] ?? 0)
      const ry = -dz(pad.axes[3] ?? 0)
      padNeckDelta = { dx: rx, dy: ry }
    } else {
      activeGamepadName.value = ''
      padVector.x = 0
      padVector.y = 0
      padNeckDelta = { dx: 0, dy: 0 }
    }
  } else {
    activeGamepadName.value = ''
    padVector.x = 0
    padVector.y = 0
    padNeckDelta = { dx: 0, dy: 0 }
  }
  rafId = window.requestAnimationFrame(pollGamepad)
}

function stopAllInputs() {
  useWanClient().releaseTalk('key')
  useWanClient().releaseTalk('pointer')
  clearTimeout(neckTrailingTimer)
  buttonVector.x = 0
  buttonVector.y = 0
  keyVector.x = 0
  keyVector.y = 0
  padVector.x = 0
  padVector.y = 0
  pressedKeys.clear()
  keySteer.value = 0
  padNeckDelta = { dx: 0, dy: 0 }
  tickDrive()
}

export function useInputController() {
  function mount() {
    if (!listenersAttached) {
      listenersAttached = true
      const wan = useWanClient()
      stopFeedbackWatch = watch(wan.state, () => {
        syncNeckFeedback()
        if (wan.state.value?.latched) mode.value = 'PARK'
      }, { immediate: true })
      window.addEventListener('keydown', onKeyDown)
      window.addEventListener('keyup', onKeyUp)
      window.addEventListener('blur', stopAllInputs)
      document.addEventListener('visibilitychange', () => {
        if (document.hidden) stopAllInputs()
      })
      driveTimer = window.setInterval(tickDrive, DRIVE_INTERVAL_MS)
      neckTimer = window.setInterval(tickPadNeck, NECK_INTERVAL_MS)
      rafId = window.requestAnimationFrame(pollGamepad)
    }
  }

  function unmount() {
    if (!listenersAttached) return
    listenersAttached = false
    stopFeedbackWatch?.()
    window.removeEventListener('keydown', onKeyDown)
    window.removeEventListener('keyup', onKeyUp)
    window.removeEventListener('blur', stopAllInputs)
    clearInterval(driveTimer)
    clearInterval(neckTimer)
    clearTimeout(neckTrailingTimer)
    cancelAnimationFrame(rafId)
  }

  function setButtonDrive(x: number, y: number) {
    buttonVector.x = clamp(x, -1, 1)
    buttonVector.y = clamp(y, -1, 1)
    tickDrive()
  }

  function setChassisMode(next: ChassisMode) {
    mode.value = next
    stopAllInputs()
    const wan = useWanClient()
    if (wan.controlling.value) {
      wan.sendDrive({ mode: next, speed_m_s: 0, steering_rad: 0, yaw_rate_rad_s: 0 })
    }
  }

  function centerHead(followEyes = false) {
    sendNeck(0, 0, true, followEyes === true)
  }

  return {
    mode,
    driveStick: computed(activeDriveAxis),
    canCenterHead, headDisabled, headStatus,
    chassisOutput: computed(() => chassisOutput.value),
    neckStick,
    neckPose,
    activeGamepadName,
    mount,
    unmount,
    setButtonDrive,
    setChassisMode,
    setNeckStick,
    centerHead,
    stopAllInputs,
  }
}
