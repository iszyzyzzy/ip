export type ChassisMode = 'ACKERMANN' | 'CRAB' | 'SPIN' | 'PARK'

export type MotionCurveType = 'SERVO_NATIVE' | 'S_CURVE' | 'S_CURVE_STRONG' | 'CUSTOM'

export type MediaPhase = 'inactive' | 'starting' | 'live' | 'offline'

export type TalkPhase = 'idle' | 'accepted' | 'completed' | 'failed' | 'offline'

export interface RemoteError {
  code: string
  message: string
}

export interface RobotCapabilities {
  audio?: boolean
  video?: boolean
  mic?: boolean
}

export interface RobotInfo {
  robot_id: string
  online: boolean
  busy: boolean
  capabilities: RobotCapabilities
}

export interface IceServerConfig {
  urls: string | string[]
  username?: string
  credential?: string
}

export interface SessionOpened {
  type: 'session.opened'
  sid: string
  robot_id: string
  capabilities: RobotCapabilities
  ice_servers: IceServerConfig[]
}

export interface TalkStatus {
  phase: TalkPhase
  code?: string
  message?: string
}

export interface NetworkStatus {
  kind: 'wifi' | 'cellular' | 'ethernet' | 'offline' | 'unknown' | string
  interface: string
  connected: boolean
  signal_percent?: number
  rssi_dbm?: number
  ssid: string
  operator: string
  technology: string
  updated_at_ms: string | number
  error: string
}

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
  audio?: Record<string, any>
  network?: NetworkStatus
}

export interface StateSnapshot {
  seq: number
  epoch: string
  dropped_drive: number
  video: MediaPhase
  mic: MediaPhase
  talk: TalkStatus
  state?: RobotState
  error?: RemoteError
}

export interface CurveKeyframe {
  t: number
  v: number
  in_t: number
  in_v: number
  out_t: number
  out_v: number
}

export interface CustomCurve {
  keyframes: CurveKeyframe[]
}

export interface SequenceEvent {
  eye_expression?: string
  sound?: string
}

export interface PresetStep {
  preset?: string
  move_ms?: number
  hold_ms: number
  event?: SequenceEvent
  motion_curve?: MotionCurveType
  custom_curve?: CustomCurve
}

export interface ActionPreset {
  move_ms: number
  targets: {
    eye_expression?: string
    left_arm?: { joint_rad: number[], duration_ms: number }
    right_arm?: { joint_rad: number[], duration_ms: number }
    neck?: { yaw_rad?: number, pitch_rad?: number }
  }
}

export interface ActionSequence {
  motion_curve?: MotionCurveType
  custom_curve?: CustomCurve
  steps: PresetStep[]
}

export interface ActionRecord {
  revision: string
  deleted: boolean
  action: {
    name: string
    duration_ms?: number
    steps?: unknown[]
    preset?: ActionPreset
    sequence?: ActionSequence
  }
}

export interface GamepadStatus {
  device: string
  error: string
  controlling: boolean
  held: string[]
  bindings: Record<string, string>
}

export type SpeedGear = 'low' | 'mid' | 'high'

export interface GearSpeeds {
  low: number
  mid: number
  high: number
}

export interface ClientConfig {
  signal_url: string
  operator_token: string
  robot_id: string
  keyboard_enabled: boolean
  gamepad_enabled: boolean
  speed_gear: SpeedGear
  gear_speeds: GearSpeeds
  drive_scale?: number
  microphone_device_id: string
  mic_listen_enabled: boolean
  auto_half_duplex: boolean
}

export interface RtcStatsSummary {
  rttMs: number | null
  candidateType: string
  videoFps: number | null
  videoWidth: number | null
  videoHeight: number | null
  bitrateKbps: number | null
}
