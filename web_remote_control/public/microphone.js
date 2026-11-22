class Microphone extends AudioWorkletProcessor {
  constructor() {
    super()
    this.samples = new Int16Array(960)
    this.used = 0
  }
  process(inputs) {
    for (const sample of inputs[0]?.[0] ?? []) {
      this.samples[this.used++] = Math.round(Math.max(-1, Math.min(1, sample)) * 32767)
      if (this.used === this.samples.length) {
        this.port.postMessage(this.samples.buffer, [this.samples.buffer])
        this.samples = new Int16Array(960)
        this.used = 0
      }
    }
    return true
  }
}
registerProcessor('microphone', Microphone)
