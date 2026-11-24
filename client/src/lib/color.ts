export function clampPercentage(percent: number): number {
  if (!Number.isFinite(percent)) return 0
  return Math.max(0, Math.min(100, percent))
}

export function interpolateColor(
  percent: number,
  startColor: string,
  endColor: string,
): string {
  const start = parseHexColor(startColor)
  const end = parseHexColor(endColor)

  if (!start || !end) {
    throw new Error('只支持 3 位、6 位或 8 位十六进制颜色')
  }

  const t = clampPercentage(percent) / 100
  const r = Math.round(start.r + (end.r - start.r) * t)
  const g = Math.round(start.g + (end.g - start.g) * t)
  const b = Math.round(start.b + (end.b - start.b) * t)
  const a = start.a + (end.a - start.a) * t

  return a === 1 ? `rgb(${r}, ${g}, ${b})` : `rgba(${r}, ${g}, ${b}, ${a.toFixed(3)})`
}

function parseHexColor(color: string): { r: number, g: number, b: number, a: number } | null {
  const hex = color.trim().replace(/^#/, '')
  if (!/^(?:[\da-f]{3}|[\da-f]{4}|[\da-f]{6}|[\da-f]{8})$/i.test(hex)) return null

  let expanded = hex
  if (hex.length === 3 || hex.length === 4) {
    expanded = [...hex].map(char => char + char).join('')
  }

  return {
    r: Number.parseInt(expanded.slice(0, 2), 16),
    g: Number.parseInt(expanded.slice(2, 4), 16),
    b: Number.parseInt(expanded.slice(4, 6), 16),
    a: expanded.length === 8
      ? Number.parseInt(expanded.slice(6, 8), 16) / 255
      : 1,
  }
}
