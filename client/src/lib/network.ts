import type { NetworkStatus } from './types'

export function networkKindLabel(kind?: string): string {
  switch (kind) {
    case 'wifi':
      return 'Wi-Fi'
    case 'cellular':
      return '4G/5G'
    case 'ethernet':
      return '有线'
    case 'offline':
      return '离线'
    default:
      return '未知'
  }
}

export function describeNetwork(net?: NetworkStatus | null) {
  if (!net) {
    return {
      kindLabel: '未知',
      percent: null as number | null,
      dbmText: '',
      identity: '',
      summary: '—',
      tone: 'normal' as 'ok' | 'warn' | 'danger' | 'normal',
    }
  }
  const kindLabel = networkKindLabel(net.kind)
  const percent = typeof net.signal_percent === 'number'
    ? Math.max(0, Math.min(100, Math.round(net.signal_percent)))
    : null
  const dbmText = typeof net.rssi_dbm === 'number' ? `${net.rssi_dbm} dBm` : ''
  const identity = net.ssid || net.operator || ''
  const tech = net.technology || kindLabel

  let summary = '离线'
  if (net.connected) {
    if (net.kind === 'ethernet') {
      summary = [kindLabel, net.interface].filter(Boolean).join(' · ')
    } else {
      summary = [tech, percent != null ? `${percent}%` : '', dbmText, identity]
        .filter(Boolean)
        .join(' · ') || kindLabel
    }
  } else if (net.error) {
    summary = net.error
  }

  let tone: 'ok' | 'warn' | 'danger' | 'normal' = 'normal'
  if (!net.connected || net.kind === 'offline') {
    tone = 'danger'
  } else if (percent != null) {
    tone = percent >= 60 ? 'ok' : percent >= 30 ? 'warn' : 'danger'
  } else if (net.kind === 'ethernet') {
    tone = 'ok'
  }

  return { kindLabel, percent, dbmText, identity, summary, tone }
}
