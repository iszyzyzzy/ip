<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useWanClient } from '@/lib/wan-client'

const wan = useWanClient()
const selectedSound = ref('')
watch(wan.sounds, names => {
  if (!names.includes(selectedSound.value)) selectedSound.value = names[0] ?? ''
}, { immediate: true })
const speakerVolume = computed(() => wan.state.value?.audio?.volume ?? 100)
async function speaker(params: { sound?: string, volume?: number }) {
  try { await wan.rpc('speaker', params) }
  catch (err) { wan.errorMessage.value = String((err as Error).message || err) }
}
</script>

<template>
  <section class="speaker-panel dv-card">
    <h2 class="dv-section-title">预制音频</h2>
    <div class="sound-row">
      <select v-model="selectedSound" class="dv-input" aria-label="预制音频" :disabled="!wan.online.value || !wan.sounds.value.length">
        <option v-if="!wan.sounds.value.length" value="">{{ wan.online.value ? "音库为空" : "等待连接" }}</option>
        <option v-for="name in wan.sounds.value" :key="name" :value="name">{{ name }}</option>
      </select>
      <button class="dv-btn dv-btn-sm" :disabled="!wan.online.value || !selectedSound" @click="speaker({ sound: selectedSound })">播放</button>
    </div>
    <label class="sound-row">音量
      <input type="range" min="0" max="100" :value="speakerVolume" :disabled="!wan.online.value" @change="speaker({ volume: Number(($event.target as HTMLInputElement).value) })">
      {{ speakerVolume }}%
    </label>
  </section>
</template>

<style scoped>
.speaker-panel { justify-content: space-between; }
h2 { margin: 0; font-size: 14px; }
.sound-row { display: flex; align-items: center; gap: 8px; font-size: 12px; white-space: nowrap; }
.sound-row select, .sound-row input { flex: 1; min-width: 0; width: 0; }
</style>
