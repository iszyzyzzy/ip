import { computed, readonly, ref, shallowRef } from 'vue'
import { clientConfig, loadClientConfig } from './settings'
import {
  connectSignal,
  disconnectSignal,
  onSignalMessage,
  onSignalStatus,
  requestSignal,
  sendSignal,
} from './signal'
import type {
  ActionRecord,
  ChassisMode,
  RobotInfo,
  RobotState,
  RemoteError,
  RtcStatsSummary,
  SessionOpened,
  StateSnapshot,
} from './types'

type ConnectionPhase = 'idle' | 'signaling' | 'negotiating' | 'connected' | 'error'

const LEASE_TTL_MS = 5000
const RENEW_INTERVAL_MS = 1200
const RPC_TIMEOUT_MS = 5000
const VIDEO_FREEZE_MS = 2200

const phase = ref<ConnectionPhase>('idle')
const signalOnline = ref(false)
const statusMessage = ref('未连接车辆')
const errorMessage = ref('')

const robots = ref<RobotInfo[]>([])
const activeSid = ref('')
const capabilities = ref<{ audio: boolean, video: boolean, mic: boolean }>({
  audio: false,
  video: false,
  mic: false,
})

const snapshot = shallowRef<StateSnapshot | null>(null)
const state = shallowRef<RobotState | null>(null)
const backendError = shallowRef<RemoteError | null>(null)
const config = shallowRef<Record<string, any> | null>(null)
const configRevision = ref('')
const actions = ref<ActionRecord[]>([])
const sounds = ref<string[]>([])

const epoch = ref('')
const busy = ref(false)
const leaseBusy = ref(false)
const talking = ref(false)
const micAvailable = ref(false)
const remoteVideoStream = shallowRef<MediaStream | null>(null)
const remoteAudioStream = shallowRef<MediaStream | null>(null)
const lastStateAt = ref(0)
const lastVideoFrameAt = ref(0)
const videoFrozen = ref(false)
const rtcStats = ref<RtcStatsSummary>({
  rttMs: null,
  candidateType: '—',
  videoFps: null,
  videoWidth: null,
  videoHeight: null,
  bitrateKbps: null,
})

let pc: RTCPeerConnection | null = null
let localAudioStream: MediaStream | null = null
let localAudioTrack: MediaStreamTrack | null = null
let rpcChannel: RTCDataChannel | null = null
let driveChannel: RTCDataChannel | null = null
let stateChannel: RTCDataChannel | null = null
let stopChannel: RTCDataChannel | null = null

let sessionGen = 0
let rpcSeq = 0
let driveSeq = 0
let lastStateSeq = 0
let epochGraceUntil = 0
let renewTimer = 0
let statsTimer = 0
let prevBytesReceived = 0
let prevStatsTime = 0
let prevFramesDecoded = 0

const rpcPending = new Map<number, {
  resolve: (val: any) => void
  reject: (err: Error) => void
  timer: number
}>()

const LEASE_METHODS = new Set([
  'renew',
  'release',
  'targets',
  'run_action',
  'cancel_action',
  'zero_neck',
])

let bridgeBound = false
function ensureSignalBridge() {
  if (bridgeBound) return
  bridgeBound = true
  onSignalStatus(st => {
    if (st.state === 'connected') {
      signalOnline.value = true
    } else {
      signalOnline.value = false
      // 遵循协议：信令断开不拆毁已建立的 WebRTC 会话，仅更新信令提示。
      if (phase.value === 'signaling' || phase.value === 'negotiating') {
        phase.value = 'error'
        errorMessage.value = st.reason || '信令连接断开'
      }
    }
  })

  onSignalMessage(async msg => {
    const type = msg.type
    if (!activeSid.value || msg.sid !== activeSid.value) return
    if (type === 'answer' && pc) {
      try {
        await pc.setRemoteDescription({ type: 'answer', sdp: msg.sdp })
      } catch (err) {
        errorMessage.value = `设置远端 SDP 失败：${String(err)}`
      }
    } else if (type === 'ice' && pc) {
      try {
        await pc.addIceCandidate(msg.candidate ?? null)
      } catch {
        // 忽略偶发迟到的候选。
      }
    } else if (type === 'session.closed') {
      const reason = msg.reason || '远程会话已结束'
      teardownRtc(`会话已关闭（${reason}）`)
    }
  })
}

function rejectAllPending(reason: string) {
  for (const waiter of rpcPending.values()) {
    clearTimeout(waiter.timer)
    waiter.reject(new Error(reason))
  }
  rpcPending.clear()
}

let micSender: RTCRtpSender | null = null

async function selectMicrophone(deviceId: string) {
  const sender = micSender
  if (!sender) return
  const stream = await navigator.mediaDevices.getUserMedia({ audio: {
    deviceId: deviceId ? { exact: deviceId } : undefined,
    channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true,
  } })
  const track = stream.getAudioTracks()[0]!
  track.enabled = false
  try {
    if (sender !== micSender) return
    await sender.replaceTrack(track)
    if (sender !== micSender) return
    track.enabled = talking.value
    localAudioStream?.getTracks().forEach(t => t.stop())
    localAudioStream = stream
    localAudioTrack = track
    micAvailable.value = true
  } finally {
    if (localAudioStream !== stream) track.stop()
  }
}

function stopLocalMic() {
  if (localAudioTrack) {
    localAudioTrack.enabled = false
  }
  localAudioStream?.getTracks().forEach(t => t.stop())
  localAudioStream = null
  localAudioTrack = null
  micAvailable.value = false
  talking.value = false
}

function clearLeaseState() {
  clearInterval(renewTimer)
  renewTimer = 0
  epoch.value = ''
  driveSeq = 0
  if (talking.value) {
    talking.value = false
    if (localAudioTrack) localAudioTrack.enabled = false
  }
}

function teardownRtc(reason = '已断开连接') {
  ++talkGeneration
  talkHolds.clear()
  sessionGen++
  clearLeaseState()
  clearInterval(statsTimer)
  statsTimer = 0
  rejectAllPending(reason)

  try { rpcChannel?.close() } catch { /* ignore */ }
  try { driveChannel?.close() } catch { /* ignore */ }
  try { stateChannel?.close() } catch { /* ignore */ }
  try { stopChannel?.close() } catch { /* ignore */ }
  rpcChannel = null
  driveChannel = null
  stateChannel = null
  stopChannel = null

  try { pc?.close() } catch { /* ignore */ }
  pc = null

  micSender = null
  stopLocalMic()
  remoteVideoStream.value = null
  remoteAudioStream.value = null
  activeSid.value = ''
  snapshot.value = null
  state.value = null
  backendError.value = null
  lastStateSeq = 0
  videoFrozen.value = false
  rtcStats.value = {
    rttMs: null,
    candidateType: '—',
    videoFps: null,
    videoWidth: null,
    videoHeight: null,
    bitrateKbps: null,
  }
  phase.value = 'idle'
  statusMessage.value = reason
}

function handleChannelMessage(raw: string) {
  let msg: any
  try {
    msg = JSON.parse(raw)
  } catch {
    return
  }
  if (msg.type === 'reply' && typeof msg.id === 'number') {
    const waiter = rpcPending.get(msg.id)
    if (!waiter) return
    rpcPending.delete(msg.id)
    clearTimeout(waiter.timer)
    if (msg.error) {
      const text = msg.error.message || msg.error.code || '请求失败'
      waiter.reject(new Error(`${msg.error.code ? msg.error.code + ': ' : ''}${text}`))
    } else {
      waiter.resolve(msg.result)
    }
  }
}

function startStatsPolling(gen: number) {
  clearInterval(statsTimer)
  prevBytesReceived = 0
  prevStatsTime = 0
  prevFramesDecoded = 0

  statsTimer = window.setInterval(async () => {
    if (gen !== sessionGen || !pc) return
    try {
      const report = await pc.getStats()
      let rttMs: number | null = null
      let candidateType = '—'
      let videoFps: number | null = null
      let videoWidth: number | null = null
      let videoHeight: number | null = null
      let bitrateKbps: number | null = null
      let localCandId = ''

      report.forEach((stat: any) => {
        if (stat.type === 'candidate-pair' && stat.state === 'succeeded' && stat.nominated) {
          if (typeof stat.currentRoundTripTime === 'number') {
            rttMs = Math.round(stat.currentRoundTripTime * 1000)
          }
          localCandId = stat.localCandidateId || ''
        }
        if (stat.type === 'inbound-rtp' && stat.kind === 'video') {
          if (typeof stat.framesPerSecond === 'number') {
            videoFps = Math.round(stat.framesPerSecond)
          }
          if (typeof stat.frameWidth === 'number') videoWidth = stat.frameWidth
          if (typeof stat.frameHeight === 'number') videoHeight = stat.frameHeight
          if (typeof stat.framesDecoded === 'number' && stat.framesDecoded > prevFramesDecoded) {
            prevFramesDecoded = stat.framesDecoded
            lastVideoFrameAt.value = Date.now()
          }
          if (typeof stat.bytesReceived === 'number' && typeof stat.timestamp === 'number') {
            if (prevStatsTime > 0 && stat.timestamp > prevStatsTime) {
              const dt = (stat.timestamp - prevStatsTime) / 1000
              bitrateKbps = Math.round(((stat.bytesReceived - prevBytesReceived) * 8) / dt / 1000)
            }
            prevBytesReceived = stat.bytesReceived
            prevStatsTime = stat.timestamp
          }
        }
      })

      if (localCandId) {
        const cand = report.get(localCandId) as any
        if (cand?.candidateType) candidateType = cand.candidateType
      }
      rtcStats.value = { rttMs, candidateType, videoFps, videoWidth, videoHeight, bitrateKbps }
      const snap = snapshot.value
      videoFrozen.value =
        snap?.video === 'live' &&
        lastVideoFrameAt.value > 0 &&
        Date.now() - lastVideoFrameAt.value > VIDEO_FREEZE_MS
    } catch {
      // 忽略统计读取瞬时异常。
    }
  }, 1000)
}

async function rpc<T = any>(
  method: string,
  params: Record<string, unknown> = {},
  channelOverride?: RTCDataChannel | null,
): Promise<T> {
  const ch = channelOverride ?? rpcChannel
  if (!ch || ch.readyState !== 'open') {
    throw new Error('控制通道未就绪')
  }
  const payload: Record<string, unknown> = { ...params }
  if (LEASE_METHODS.has(method) && payload.epoch === undefined) {
    if (!epoch.value) throw new Error('尚未取得机器人控制权')
    payload.epoch = epoch.value
  }
  const id = ++rpcSeq
  return new Promise<T>((resolve, reject) => {
    const timer = window.setTimeout(() => {
      rpcPending.delete(id)
      reject(new Error(`命令 ${method} 超时`))
    }, RPC_TIMEOUT_MS)
    rpcPending.set(id, {
      resolve: value => {
        if (method === 'update_config') clearLeaseState()
        resolve(value)
      },
      reject,
      timer,
    })
    try {
      ch.send(JSON.stringify({ id, method, params: payload }))
    } catch (err) {
      if (rpcPending.delete(id)) {
        clearTimeout(timer)
        reject(new Error(`发送 ${method} 失败：${String(err)}`))
      }
    }
  })
}

async function refreshMetadata() {
  if (!rpcChannel || rpcChannel.readyState !== 'open') return
  await Promise.all([loadConfig(), loadActions(), loadSounds()]).catch(() => {})
}

async function loadConfig() {
  try {
    const res = await rpc<{ config: Record<string, any>, revision: string }>('get_config')
    config.value = res.config
    configRevision.value = res.revision
  } catch (err) {
    errorMessage.value = String((err as Error).message || err)
  }
}

async function loadActions() {
  try {
    const res = await rpc<{ actions: ActionRecord[] }>('list_actions')
    actions.value = res.actions ?? []
  } catch (err) {
    errorMessage.value = String((err as Error).message || err)
  }
}

async function loadSounds() {
  try {
    const res = await rpc<{ names: string[] }>('list_sounds')
    sounds.value = res.names ?? []
  } catch {
    // 音库接口失败时不阻断主流程。
  }
}

async function refreshRobots() {
  await loadClientConfig()
  ensureSignalBridge()
  if (!signalOnline.value) {
    await connectSignal(clientConfig.signal_url, clientConfig.operator_token)
  }
  const res = await requestSignal<{ robots: RobotInfo[] }>({ type: 'robots.list' })
  robots.value = res.robots ?? []
  return robots.value
}

async function connect() {
  if (busy.value) return
  busy.value = true
  errorMessage.value = ''
  try {
    await loadClientConfig()
    ensureSignalBridge()
    teardownRtc('正在重新建立连接…')
    const gen = ++sessionGen

    phase.value = 'signaling'
    statusMessage.value = '正在连接公网信令…'
    await connectSignal(clientConfig.signal_url, clientConfig.operator_token)
    if (gen !== sessionGen) return

    const listResp = await requestSignal<{ robots: RobotInfo[] }>({ type: 'robots.list' })
    robots.value = listResp.robots ?? []
    const target = robots.value.find(r => r.robot_id === clientConfig.robot_id)
    if (target && !target.online) {
      throw new Error(`车辆 ${clientConfig.robot_id} 当前离线`)
    }

    statusMessage.value = '正在申请远程会话…'
    const opened = await requestSignal<SessionOpened>({
      type: 'session.open',
      robot_id: clientConfig.robot_id,
      version: 1,
    })
    if (gen !== sessionGen) return
    activeSid.value = opened.sid
    capabilities.value = {
      audio: !!opened.capabilities?.audio,
      video: !!opened.capabilities?.video,
      mic: !!opened.capabilities?.mic,
    }

    phase.value = 'negotiating'
    statusMessage.value = '正在建立 WebRTC 通道…'
    const peer = new RTCPeerConnection({ iceServers: opened.ice_servers ?? [] })
    pc = peer

    micSender = peer.addTransceiver('audio', { direction: 'sendrecv' }).sender
    try {
      await selectMicrophone(clientConfig.microphone_device_id)
    } catch {
      micAvailable.value = false
    }
    if (gen !== sessionGen) return

    peer.addTransceiver('video', { direction: 'recvonly' })

    rpcChannel = peer.createDataChannel('rpc.v1', { ordered: true })
    driveChannel = peer.createDataChannel('drive.v1', { ordered: false, maxRetransmits: 0 })
    stateChannel = peer.createDataChannel('state.v1', { ordered: false, maxRetransmits: 0 })
    stopChannel = peer.createDataChannel('stop.v1', { ordered: true })

    rpcChannel.onopen = async () => {
      if (gen !== sessionGen) return
      await refreshMetadata()
      if (gen === sessionGen) {
        await acquire(true)
      }
    }
    rpcChannel.onmessage = e => handleChannelMessage(e.data)
    stopChannel.onmessage = e => handleChannelMessage(e.data)

    stateChannel.onmessage = e => {
      if (gen !== sessionGen) return
      let snap: StateSnapshot
      try {
        snap = JSON.parse(e.data)
      } catch {
        return
      }
      if (typeof snap.seq !== 'number' || snap.seq <= lastStateSeq) return
      lastStateSeq = snap.seq
      const oldConfigRev = state.value?.config_revision
      snapshot.value = snap
      state.value = snap.state ?? null
      backendError.value = snap.error ?? null
      lastStateAt.value = Date.now()

      if (epoch.value && Date.now() > epochGraceUntil && snap.epoch !== epoch.value) {
        clearLeaseState()
        errorMessage.value = '控制租约已被车端释放或过期'
      }
      if (oldConfigRev && snap.state?.config_revision && oldConfigRev !== snap.state.config_revision) {
        loadConfig()
      }
    }

    peer.ontrack = event => {
      if (gen !== sessionGen) return
      const stream = event.streams[0] ?? new MediaStream([event.track])
      if (event.track.kind === 'video') {
        remoteVideoStream.value = stream
        lastVideoFrameAt.value = Date.now()
      } else if (event.track.kind === 'audio') {
        remoteAudioStream.value = stream
      }
    }

    let offerSent = false
    const iceQueue: (RTCIceCandidateInit | null)[] = []

    peer.onicecandidate = event => {
      if (gen !== sessionGen || !activeSid.value) return
      const candidate = event.candidate ? event.candidate.toJSON() : null
      if (!offerSent) {
        iceQueue.push(candidate)
        return
      }
      sendSignal({
        type: 'ice',
        sid: activeSid.value,
        candidate,
      }).catch(() => {})
    }

    peer.onconnectionstatechange = () => {
      if (gen !== sessionGen) return
      if (peer.connectionState === 'connected') {
        phase.value = 'connected'
        statusMessage.value = `已连接 ${clientConfig.robot_id}`
        startStatsPolling(gen)
      } else if (peer.connectionState === 'failed' || peer.connectionState === 'closed') {
        teardownRtc('WebRTC 连接已断开')
      }
    }

    const offer = await peer.createOffer()
    await peer.setLocalDescription(offer)
    await sendSignal({
      type: 'offer',
      sid: opened.sid,
      sdp: peer.localDescription?.sdp ?? offer.sdp,
    })
    offerSent = true
    for (const candidate of iceQueue) {
      if (gen !== sessionGen || !activeSid.value) break
      await sendSignal({
        type: 'ice',
        sid: activeSid.value,
        candidate,
      }).catch(() => {})
    }
  } catch (err) {
    const text = (err as Error).message || String(err)
    teardownRtc('连接失败')
    phase.value = 'error'
    errorMessage.value = text
  } finally {
    busy.value = false
  }
}

async function disconnect() {
  const sid = activeSid.value
  if (epoch.value) {
    await release().catch(() => {})
  }
  if (sid && signalOnline.value) {
    await sendSignal({ type: 'session.close', sid }).catch(() => {})
  }
  teardownRtc('已断开连接')
  await disconnectSignal()
}

async function acquire(auto = false) {
  if (epoch.value || leaseBusy.value) return
  leaseBusy.value = true
  if (!auto) errorMessage.value = ''
  try {
    const res = await rpc<{ epoch: string, ttl_ms: number }>('acquire', {
      ttl_ms: LEASE_TTL_MS,
      clear_latch: auto ? false : !!state.value?.latched,
    })
    epoch.value = res.epoch
    epochGraceUntil = Date.now() + 800
    driveSeq = 0
    clearInterval(renewTimer)
    renewTimer = window.setInterval(async () => {
      if (!epoch.value) return
      try {
        await rpc('renew', { epoch: epoch.value })
      } catch (err) {
        if (!epoch.value) return
        clearLeaseState()
        errorMessage.value = `控制续租失败：${(err as Error).message}`
      }
    }, RENEW_INTERVAL_MS)
  } catch (err) {
    if (!auto) {
      errorMessage.value = `接管失败：${(err as Error).message}`
    }
  } finally {
    leaseBusy.value = false
  }
}

async function release() {
  const currentEpoch = epoch.value
  if (!currentEpoch) return
  if (talkHolds.size || talking.value) {
    await endTalk().catch(() => {})
  }
  sendDrive({ mode: 'PARK', speed_m_s: 0, steering_rad: 0, yaw_rate_rad_s: 0 })
  clearLeaseState()
  try {
    await rpc('release', { epoch: currentEpoch })
  } catch {
    // 若通道已断，车端租约到期后会自动停止并释放。
  }
}

async function emergencyStop() {
  clearLeaseState()
  try {
    await rpc('stop', { latch: true }, stopChannel)
  } catch (err) {
    errorMessage.value = `急停命令发送失败：${(err as Error).message}`
  }
}

function sendDrive(chassis: {
  mode: ChassisMode
  speed_m_s: number
  steering_rad: number
  yaw_rate_rad_s: number
}) {
  if (!epoch.value || !driveChannel || driveChannel.readyState !== 'open') return
  driveSeq += 1
  try {
    driveChannel.send(JSON.stringify({
      epoch: epoch.value,
      seq: driveSeq,
      chassis,
    }))
  } catch {
    // 不可靠通道允许偶发发送失败，下一帧 20Hz 继续覆盖。
  }
}

async function setTargets(targets: Record<string, unknown>) {
  return rpc('targets', { targets })
}

async function runAction(name: string) {
  return rpc('run_action', { name })
}

async function cancelAction() {
  return rpc('cancel_action')
}

const talkHolds = new Set<'key' | 'pointer'>()
let talkGeneration = 0

async function holdTalk(source: 'key' | 'pointer') {
  if (phase.value !== 'connected' || talkHolds.has(source)) return
  talkHolds.add(source)
  if (talkHolds.size > 1) return
  const gen = ++talkGeneration
  try {
    await beginTalk(gen)
  } catch (err) {
    if (gen === talkGeneration) {
      talkHolds.clear()
      errorMessage.value = String((err as Error).message || err)
    }
  }
}

function releaseTalk(source: 'key' | 'pointer') {
  if (talkHolds.delete(source) && !talkHolds.size) void endTalk()
}

async function beginTalk(gen: number) {
  if (!micAvailable.value || !localAudioTrack) {
    throw new Error('本机未检测到可用麦克风权限')
  }
  errorMessage.value = ''
  await rpc('talk.begin', {})
  if (gen !== talkGeneration || !localAudioTrack) return
  localAudioTrack.enabled = true
  talking.value = true
}

async function endTalk() {
  ++talkGeneration
  talkHolds.clear()
  if (localAudioTrack) localAudioTrack.enabled = false
  talking.value = false
  try {
    await rpc('talk.end', {})
  } catch (err) {
    errorMessage.value = `结束喊话提示：${(err as Error).message}`
  }
}

export function useWanClient() {
  return {
    phase: readonly(phase),
    signalOnline: readonly(signalOnline),
    statusMessage: readonly(statusMessage),
    errorMessage,
    robots: readonly(robots),
    activeSid: readonly(activeSid),
    capabilities: readonly(capabilities),
    snapshot: readonly(snapshot),
    state: readonly(state),
    backendError: readonly(backendError),
    config: computed(() => config.value),
    configRevision: readonly(configRevision),
    actions: computed(() => actions.value),
    sounds: computed(() => sounds.value),
    epoch: readonly(epoch),
    controlling: computed(() => !!epoch.value),
    online: computed(() => phase.value === 'connected'),
    busy: computed(() => busy.value || leaseBusy.value),
    talking: readonly(talking),
    openVoiceChannel(backend: string) {
      if (!pc || phase.value !== 'connected') throw new Error('请先连接机器人')
      return pc.createDataChannel('voice.v1', { ordered: true, protocol: backend })
    },
    selectMicrophone,
    micAvailable: readonly(micAvailable),
    remoteVideoStream: readonly(remoteVideoStream),
    remoteAudioStream: readonly(remoteAudioStream),
    lastStateAt: readonly(lastStateAt),
    lastVideoFrameAt: readonly(lastVideoFrameAt),
    videoFrozen: readonly(videoFrozen),
    rtcStats: readonly(rtcStats),
    connect,
    disconnect,
    refreshRobots,
    acquire,
    release,
    emergencyStop,
    sendDrive,
    setTargets,
    runAction,
    cancelAction,
    holdTalk, releaseTalk,
    endTalk,
    rpc,
    loadConfig,
    loadActions,
    loadSounds,
  }
}
