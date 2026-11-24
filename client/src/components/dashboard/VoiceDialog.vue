<script setup lang="ts">
import { ref, onBeforeUnmount } from 'vue'
import { clientConfig } from '@/lib/settings'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()
const dialog = ref<HTMLDialogElement>()
const stage = ref<'idle' | 'connecting' | 'recording' | 'waiting'>('idle')
const backend = ref(localStorage.getItem('voice.tts') || 'local')
const question = ref('')
const answer = ref('')
const error = ref('')
let channel: RTCDataChannel | null = null
let context: AudioContext | null = null
let stream: MediaStream | null = null
let timer: number | undefined

function stopMic() {
  clearTimeout(timer)
  stream?.getTracks().forEach(track => track.stop())
  stream = null
  if (context) void context.close()
  context = null
}
function reset() {
  const old = channel
  channel = null
  old?.close()
  stopMic()
  stage.value = 'idle'
}
function finish() {
  if (stage.value !== 'recording') return
  stopMic()
  stage.value = 'waiting'
  channel?.send('end')
}
async function start() {
  reset()
  question.value = answer.value = error.value = ''
  stage.value = 'connecting'
  try {
    const link = wan.openVoiceChannel(backend.value)
    channel = link
    const fail = (reason: string) => {
      if (channel !== link) return
      error.value = reason
      reset()
    }
    timer = window.setTimeout(() => fail('车载语音服务未就绪'), 12000)
    link.onclose = () => fail('对话连接已关闭')
    link.onerror = () => fail('对话连接出错')
    link.onmessage = async ({ data }) => {
      if (channel !== link) return
      try {
        if (data === 'ready') {
          clearTimeout(timer)
          const device = clientConfig.microphone_device_id
          const mic = await navigator.mediaDevices.getUserMedia({ audio: {
            channelCount: 1, echoCancellation: true, deviceId: device ? { exact: device } : undefined,
          } })
          if (channel !== link) { mic.getTracks().forEach(t => t.stop()); return }
          stream = mic
          const ctx = new AudioContext({ sampleRate: 16000 })
          context = ctx
          await ctx.resume()
          if (channel !== link) return
          const source = ctx.createMediaStreamSource(mic)
          const processor = ctx.createScriptProcessor(1024, 1, 1)
          source.connect(processor)
          processor.connect(ctx.destination)
          processor.onaudioprocess = ({ inputBuffer }) => {
            if (channel !== link || stage.value !== 'recording') return
            if (link.bufferedAmount > 64000) { fail('网络拥塞，请重试'); return }
            const samples = inputBuffer.getChannelData(0)
            link.send(Int16Array.from(samples, v => Math.round(Math.max(-1, Math.min(1, v)) * 32767)).buffer)
          }
          stage.value = 'recording'
          timer = window.setTimeout(finish, 59000)
        } else {
          const msg = JSON.parse(data)
          if (msg.error) fail(msg.error)
          if (msg.partial || msg.transcript) question.value = msg.transcript || msg.partial
          if (msg.answer) answer.value = msg.answer
          if (msg.done) reset()
        }
      } catch (err) { fail(String(err)) }
    }
  } catch (err) { error.value = String(err); reset() }
}
function localStorageSave() { localStorage.setItem('voice.tts', backend.value) }
onBeforeUnmount(reset)
</script>

<template>
  <button class="dv-btn dv-btn-sm" :disabled="!wan.online.value" @click="dialog?.showModal()">AI 对话</button>
  <dialog ref="dialog" @close="reset">
    <header><strong>AI 语音对话</strong><button class="dv-btn dv-btn-sm" @click="dialog?.close()">关闭</button></header>
    <label>合成后端 <select v-model="backend" class="dv-input" :disabled="stage !== 'idle'"
      @change="localStorageSave"><option value="local">车载本地</option><option value="deepseek">DeepSeek</option><option value="doubao">豆包 TTS 3.0</option><option value="xfyun">讯飞</option></select></label>
    <p v-if="question"><b>我</b>　{{ question }}</p>
    <p v-if="answer"><b>机器人</b>　{{ answer }}</p>
    <p v-if="error" role="alert">{{ error }}</p>
    <button class="dv-btn" :disabled="stage === 'connecting' || stage === 'waiting' || !wan.online.value"
      @click="stage === 'recording' ? finish() : start()">
      {{ { idle: '开始说话', connecting: '正在连接…', recording: '说完了', waiting: answer ? '正在播报…' : '正在回答…' }[stage] }}
    </button>
  </dialog>
</template>

<style scoped>
dialog { background: #091c35; color: var(--text); border: 1px solid var(--cyan-bright); width: min(420px, 85vw); padding: 18px; }
dialog::backdrop { background: #0008; }
header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; }
p { line-height: 1.6; white-space: pre-wrap; }
</style>
