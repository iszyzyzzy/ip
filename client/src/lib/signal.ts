import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

type SignalHandler = (msg: Record<string, any>) => void
type StatusHandler = (status: { state: string, reason: string }) => void

let seqId = 0
let unlistenMsg: UnlistenFn | null = null
let unlistenStatus: UnlistenFn | null = null
let initialized = false

const pending = new Map<string, {
  resolve: (value: any) => void
  reject: (err: Error) => void
  timer: number
}>()
const messageListeners = new Set<SignalHandler>()
const statusListeners = new Set<StatusHandler>()

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

async function ensureListeners() {
  if (initialized || !isTauriRuntime()) return
  initialized = true
  unlistenMsg = await listen<string>('signal:message', ({ payload }) => {
    let msg: Record<string, any>
    try {
      msg = JSON.parse(payload)
    } catch {
      return
    }
    const id = typeof msg.id === 'string' ? msg.id : null
    if (id && pending.has(id)) {
      const waiter = pending.get(id)!
      pending.delete(id)
      clearTimeout(waiter.timer)
      if (msg.type === 'error') {
        const errMsg = msg.error?.message || msg.error?.code || '信令请求失败'
        waiter.reject(new Error(`${msg.error?.code ? msg.error.code + ': ' : ''}${errMsg}`))
      } else {
        waiter.resolve(msg)
      }
    }
    for (const fn of messageListeners) fn(msg)
  })
  unlistenStatus = await listen<{ state: string, reason: string }>('signal:status', ({ payload }) => {
    if (payload.state === 'closed') {
      for (const waiter of pending.values()) {
        clearTimeout(waiter.timer)
        waiter.reject(new Error(payload.reason || '信令连接已断开'))
      }
      pending.clear()
    }
    for (const fn of statusListeners) fn(payload)
  })
}

export function onSignalMessage(fn: SignalHandler): () => void {
  messageListeners.add(fn)
  return () => messageListeners.delete(fn)
}

export function onSignalStatus(fn: StatusHandler): () => void {
  statusListeners.add(fn)
  return () => statusListeners.delete(fn)
}

export async function connectSignal(url: string, token: string): Promise<void> {
  if (!isTauriRuntime()) {
    throw new Error('当前非 Tauri 桌面环境，请使用 pnpm tauri dev 启动桌面客户端')
  }
  await ensureListeners()
  await invoke('signal_connect', { url, token })
}

export async function disconnectSignal(): Promise<void> {
  for (const waiter of pending.values()) {
    clearTimeout(waiter.timer)
    waiter.reject(new Error('信令已断开'))
  }
  pending.clear()
  if (isTauriRuntime()) {
    await invoke('signal_disconnect').catch(() => {})
  }
}

export async function sendSignal(msg: Record<string, unknown>): Promise<void> {
  if (!isTauriRuntime()) throw new Error('非 Tauri 环境')
  await invoke('signal_send', { payload: JSON.stringify(msg) })
}

export async function requestSignal<T = any>(
  msg: Record<string, unknown>,
  timeoutMs = 8000,
): Promise<T> {
  if (!isTauriRuntime()) throw new Error('非 Tauri 环境')
  const id = `sig-${++seqId}`
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      pending.delete(id)
      reject(new Error(`信令请求 ${String(msg.type ?? '')} 超时`))
    }, timeoutMs)
    pending.set(id, { resolve, reject, timer })
    invoke('signal_send', { payload: JSON.stringify({ ...msg, id }) }).catch(err => {
      if (pending.delete(id)) {
        clearTimeout(timer)
        reject(new Error(String(err)))
      }
    })
  })
}

export function cleanupSignalBridge() {
  unlistenMsg?.()
  unlistenStatus?.()
  unlistenMsg = null
  unlistenStatus = null
  initialized = false
}
