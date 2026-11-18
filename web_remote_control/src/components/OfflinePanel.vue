<script setup lang="ts">
defineProps<{ offline: boolean, name: string }>()
</script>

<template>
  <div class="relative">
    <div :inert="offline" :class="{ 'opacity-35': offline }"><slot /></div>
    <div v-if="offline" class="absolute inset-0 z-[1] flex flex-col items-center justify-start rounded-xl bg-background/55">
      <div role="status" class="sticky top-[35vh] my-auto rounded-lg border bg-background px-4 py-2 text-center text-sm shadow-sm">
        {{ name }}组件不在线
      </div>
    </div>
  </div>
</template>
