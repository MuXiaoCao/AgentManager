export interface SessionEntry {
  session_id: string
  agent: string
  cwd: string
  iterm_session_id: string
  last_event: string
  last_updated: string
  notification_count: number
  alias: string | null
}

export interface HookStatus {
  script_installed: boolean
  settings_exists: boolean
  installed_events: string[]
  expected_command: string
}

export interface HookInstallReport {
  script_path: string
  settings_path: string
  added_events: string[]
  command: string
}

export interface ArrangeReport {
  arranged: number
  skipped: number
  cols: number
  rows: number
}

export interface ClaudeHistoryEntry {
  session_id: string
  cwd: string
  project: string
  started_at: string | null
  kind: string
  summary: string
  size_bytes: number
}
