export interface CapabilityStatus {
  supported: boolean
  available: boolean
  detail?: string | null
}

export interface PlatformLinuxInfo {
  desktop: string
  session_type: string
  xdg_current_desktop: string
  desktop_session: string
  has_wayland: boolean
  has_x11: boolean
  has_sway_ipc: boolean
}

export interface PlatformCapabilities {
  key_interception: CapabilityStatus
  literal_injection: CapabilityStatus
  layout_detection: CapabilityStatus
  system_actions: CapabilityStatus
}

export interface PlatformInfo {
  os: string
  linux?: PlatformLinuxInfo | null
  capabilities: PlatformCapabilities
}
