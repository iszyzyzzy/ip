import { computed, readonly, ref, shallowRef } from 'vue'

export type ChassisMode = 'ACKERMANN' | 'CRAB' | 'SPIN' | 'PARK'

export interface RemoteError { code: string, message: string }

export interface RobotState {
  revision: string
  config_revision: string
  applied_config_revision: string
  simulated: boolean
  stopped: boolean
  latched: boolean
  owner: string
  lease_remaining_ms: number
  action: string
  fault: string
  devices: Record<string, any>
}

export interface ActionRecord {
  revision: string
  deleted: boolean
  action: { name: string, duration_ms: number, steps: unknown[] }
}

type Link = 'idle' | 'connecting' | 'auth' | 'ready' | 'closed'

const TOKEN_KEY = 'robot-remote-token'
// 网关建议每约 300 ms 续期；租约 1 s 留出两次续期失败的余量，掉线后机器人很快自停。
const LEASE_TTL_MS = 1000
const RENEW_MS = 300
const COMMAND_TIMEOUT_MS = 2500

class CommandError extends Error {
  code: string
  constructor(error: RemoteError) {
    super(error.message)
    this.code = error.code
  }
}

const link = ref<Link>('idle')
const state = shallowRef<RobotState | null>(null)
const backendError = shallowRef<RemoteError | null>(null)
const lastStateAt = ref(0)
const controlling = ref(false)
const busy = ref(false)
const message = ref('')
const token = ref(loadToken())
const needToken = ref(false)

let ws: WebSocket | null = null
let nextId = 0
let renewTimer = 0
let reconnectTimer = 0
let reconnectDelay = 500
const pending = new Map<number, { resolve: (v: any) => void, reject: (e: Error) => void, timer: number }>()

function loadToken() {
  try { return localStorage.getItem(TOKEN_KEY) ?? '' } catch { return '' }
}

function wsUrl() {
  const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${scheme}//${location.host}/ws`
}

function failPending(reason: string) {
  for (const waiter of pending.values()) {
    clearTimeout(waiter.timer)
    waiter.reject(new CommandError({ code: 'UNAVAILABLE', message: reason }))
  }
  pending.clear()
}

function send<T = any>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  return new Promise((resolve, reject) => {
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      reject(new CommandError({ code: 'UNAVAILABLE', message: '遥控连接未就绪' }))
      return
    }
    const id = ++nextId
    // 超时只表示结果不确定；运动命令不重试，由下一帧摇杆或状态推送决定后续行为。
    const timer = window.setTimeout(() => {
      pending.delete(id)
      reject(new CommandError({ code: 'DEADLINE_EXCEEDED', message: '命令超时' }))
    }, COMMAND_TIMEOUT_MS)
    pending.set(id, { resolve, reject, timer })
    ws.send(JSON.stringify({ id, method, params }))
  })
}

function scheduleReconnect() {
  clearTimeout(reconnectTimer)
  reconnectTimer = window.setTimeout(connect, reconnectDelay)
  reconnectDelay = Math.min(reconnectDelay * 2, 5000)
}

function connect() {
  clearTimeout(reconnectTimer)
  if (ws && ws.readyState <= WebSocket.OPEN) return
  link.value = 'connecting'
  const socket = new WebSocket(wsUrl())
  ws = socket

  socket.onmessage = async ({ data }) => {
    const msg = JSON.parse(data)
    if (msg.type === 'ready') {
      reconnectDelay = 500
      if (msg.authenticated) {
        link.value = 'ready'
        needToken.value = false
        return
      }
      link.value = 'auth'
      if (!token.value) {
        needToken.value = true
        return
      }
      try {
        await send('auth', { token: token.value })
        link.value = 'ready'
        needToken.value = false
      } catch {
        // 凭据错误时服务端会关闭连接；停止自动重连，等待用户重新输入。
        needToken.value = true
      }
    } else if (msg.type === 'reply') {
      const waiter = pending.get(msg.id)
      if (!waiter) return
      pending.delete(msg.id)
      clearTimeout(waiter.timer)
      if (msg.error) waiter.reject(new CommandError(msg.error))
      else waiter.resolve(msg.result)
    } else if (msg.type === 'state') {
      state.value = msg.state
      backendError.value = null
      lastStateAt.value = Date.now()
    } else if (msg.type === 'backend_error') {
      backendError.value = msg.error
    }
  }

  socket.onclose = (event) => {
    if (ws !== socket) return
    ws = null
    failPending('遥控连接已断开')
    // 网关在断线时会释放本连接的会话；本地同步清除，重连后不自动取回控制权。
    dropControl()
    link.value = 'closed'
    if (event.code === 1008) {
      needToken.value = true
      message.value = '访问凭据无效'
      return
    }
    scheduleReconnect()
  }
}

function setToken(value: string) {
  token.value = value
  try { localStorage.setItem(TOKEN_KEY, value) } catch { /* 隐私模式下仅本次有效 */ }
  needToken.value = false
  if (ws) ws.close()
  else connect()
}

function dropControl() {
  clearInterval(renewTimer)
  renewTimer = 0
  controlling.value = false
}

async function acquire() {
  if (controlling.value || busy.value) return
  busy.value = true
  try {
    // 锁定停止只能由用户在此主动解除，网页不在后台自动清除。
    await send('acquire', { owner: 'web-remote', ttl_ms: LEASE_TTL_MS,
                            clear_latch: !!state.value?.latched })
    controlling.value = true
    message.value = ''
    renewTimer = window.setInterval(async () => {
      try {
        await send('renew')
      } catch (error) {
        if (!controlling.value) return
        dropControl()
        message.value = `控制权已失效：${(error as Error).message}`
      }
    }, RENEW_MS)
  } catch (error) {
    message.value = `无法取得控制权：${(error as Error).message}`
  } finally {
    busy.value = false
  }
}

async function release() {
  if (!controlling.value) return
  dropControl()
  try { await send('release') } catch { /* 租约到期后 robotd 会自行停止 */ }
}

async function stop(latch = true) {
  dropControl()
  navigator.vibrate?.(80)
  try {
    await send('stop', { latch })
  } catch (error) {
    message.value = `急停命令失败：${(error as Error).message}`
  }
}

// 目标发送丢弃排队：慢网络下只发最新输入，避免旧速度在之后才到达。
let targetInFlight = false
async function targets(value: Record<string, unknown>, force = false) {
  if (!controlling.value) return
  if (targetInFlight && !force) return
  targetInFlight = true
  try {
    await send('targets', { targets: value })
  } catch (error) {
    message.value = (error as Error).message
  } finally {
    targetInFlight = false
  }
}

async function command<T = any>(method: string, params: Record<string, unknown> = {}) {
  try {
    return await send<T>(method, params)
  } catch (error) {
    message.value = (error as Error).message
    throw error
  }
}

// 切到后台时浏览器会节流定时器，摇杆也无法松开；主动释放比依赖租约到期更快停下。
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden') release()
})
window.addEventListener('pagehide', () => { release() })

export function useRemote() {
  return {
    link: readonly(link),
    state,
    backendError,
    lastStateAt: readonly(lastStateAt),
    controlling: readonly(controlling),
    busy: readonly(busy),
    message,
    token: readonly(token),
    needToken: readonly(needToken),
    online: computed(() => link.value === 'ready'),
    connect,
    setToken,
    acquire,
    release,
    stop,
    targets,
    command,
  }
}
