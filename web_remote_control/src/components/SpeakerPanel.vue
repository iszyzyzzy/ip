<script setup lang="ts">
import { ref, watch, onUnmounted } from 'vue'
import { useRemote } from '@/lib/remote'
import { Button } from '@/components/ui/button'
const remote = useRemote()
const sounds = ref<string[]>([])
const selected = ref('')
const status = ref('')
const talking = ref(false)
const dialogue = ref(false)
const listening = ref(false)
const waiting = ref(false)
const playing = ref(false)
const volume = ref(100)
let appliedVolume = 100
const volumeReady = ref(false)
let generation = 0
let stream: MediaStream | undefined
let context: AudioContext | undefined
let socket: WebSocket | undefined
async function api(path: string, method = 'GET', body?: string) {
  const result = await fetch('/api/audio/' + path, { method, body, headers: {Authorization: 'Bearer ' + remote.token.value, 'Content-Type': 'application/json'} })
  const data = await result.json()
  if (!result.ok) throw Error(data.error?.message || result.statusText)
  return data
}
watch(remote.online, async online => {
  volumeReady.value = false
  if (!online) { stop(); return }
  try {
    volume.value = appliedVolume = await api('volume')
    volumeReady.value = true
    sounds.value = await api('presets')
    selected.value = sounds.value[0] || ''
  }
  catch (e) { status.value = String(e) }
}, { immediate: true })
async function saveVolume() {
  volumeReady.value = false
  try { appliedVolume = await api('volume', 'PUT', JSON.stringify(volume.value)) }
  catch (e) { status.value = String(e); volume.value = appliedVolume }
  finally { volumeReady.value = true }
}
async function play() {
  playing.value = true
  try { const r = await api('presets/' + encodeURIComponent(selected.value), 'POST'); status.value = r.phase === 'completed' ? '播放完毕' : r.phase }
  catch (e) { status.value = String(e) }
  finally { playing.value = false }
}
function releaseMic() {
  stream?.getTracks().forEach(track => track.stop())
  stream = undefined
  if (context) void context.close()
  context = undefined
}
function stop() {
  generation++
  releaseMic()
  if (!dialogue.value && socket?.readyState === WebSocket.OPEN) socket.send('end')
  else socket?.close()
  socket = undefined
  talking.value = false
  dialogue.value = false
  listening.value = false
  waiting.value = false
}
async function start(voice = false) {
  const id = ++generation
  talking.value = true
  dialogue.value = voice
  status.value = '正在连接麦克风'
  try {
    if (!navigator.mediaDevices?.getUserMedia) throw Error('麦克风需要通过受信任的 HTTPS 或 localhost 打开遥控器')
    const mic = await navigator.mediaDevices.getUserMedia({audio: {channelCount: 1, echoCancellation: true}})
    if (id !== generation) { mic.getTracks().forEach(t => t.stop()); return }
    stream = mic
    const ctx = context = new AudioContext({sampleRate: voice ? 16000 : 48000})
    await ctx.resume()
    await ctx.audioWorklet.addModule('/microphone.js')
    if (id !== generation) return
    const ws = socket = new WebSocket(location.origin.replace(/^http/, 'ws') + (voice ? '/voice' : '/talk'))
    ws.onopen = () => ws.send(JSON.stringify({token: remote.token.value}))
    ws.onmessage = ({data}) => {
      if (id !== generation) return
      if (voice && data !== 'ready') {
        const msg = JSON.parse(data)
        if (msg.error) { status.value = msg.error; stop() }
        else if (msg.done) { stop() }
        else if (msg.answer) status.value = '回答：' + msg.answer
        else if (msg.transcript) status.value = '已识别：' + msg.transcript
        else if (msg.partial) status.value = msg.partial
        return
      }
      if (data !== 'ready') { status.value = data; stop(); return }
      const node = new AudioWorkletNode(ctx, 'microphone')
      ctx.createMediaStreamSource(mic).connect(node)
      node.connect(ctx.destination)
      node.port.onmessage = event => {
        if (id !== generation || waiting.value || ws.readyState !== WebSocket.OPEN) return
        if (ws.bufferedAmount > 24000) { stop(); status.value = '网络拥堵，喊话已结束'; return }
        ws.send(event.data)
      }
      listening.value = true
      status.value = voice ? '正在聆听' : '喊话中'
    }
    ws.onclose = () => { if (id === generation) { stop(); status.value = '喊话连接已关闭' } }
  } catch (e) { if (id === generation) { stop(); status.value = String(e) } }
}
function endTalk() { stop(); status.value = '喊话已结束' }
function submitVoice() {
  if (socket?.readyState !== WebSocket.OPEN || !context || waiting.value) { stop(); return }
  waiting.value = true
  releaseMic()
  socket.send('end')
  status.value = '正在处理…'
}
function hidden() { if (document.hidden && talking.value) endTalk() }
document.addEventListener('visibilitychange', hidden)
onUnmounted(() => { document.removeEventListener('visibilitychange', hidden); stop() })
</script>
<template>
  <section v-if="remote.online.value" class="flex flex-wrap items-center gap-3 rounded-xl border p-3">
    <label>音响 <select v-model="selected" class="rounded border p-2"><option v-for="s in sounds" :key="s">{{ s }}</option></select></label>
    <Button variant="secondary" :disabled="!selected || playing || talking" @click="play">播放</Button>
    <Button v-if="playing" variant="secondary" @click="remote.command('stop_audio').catch(() => {})">停止播放</Button>
    <Button :disabled="playing || dialogue" :variant="talking && !dialogue ? 'destructive' : 'default'" @click="talking ? endTalk() : start()">{{ talking && !dialogue ? '结束喊话' : '开始喊话' }}</Button>
    <Button :disabled="playing || (talking && !dialogue) || (dialogue && !listening) || waiting" variant="secondary" @click="dialogue ? submitVoice() : start(true)">{{ waiting ? '回答中…' : dialogue ? '说完了' : '语音对话' }}</Button>
    <span role="status" class="text-sm">{{ status }}</span>
    <label class="flex w-full items-center gap-3 text-sm">
      音量
      <input v-model.number="volume" type="range" min="0" max="100" step="1" class="h-10 min-w-0 flex-1" :disabled="!volumeReady" @change="saveVolume">
      <span class="w-10 text-right tabular-nums">{{ volume }}%</span>
    </label>
  </section>
</template>
