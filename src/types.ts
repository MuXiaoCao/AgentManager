export interface SessionEntry {
  session_id: string
  agent: string
  cwd: string
  iterm_session_id: string
  last_event: string
  last_updated: string
  notification_count: number
  alias: string | null
  preview: string
}

export type AgentName = 'claude' | 'codex'

export interface AgentHookStatus {
  agent: AgentName
  script_installed: boolean
  settings_exists: boolean
  installed_events: string[]
  expected_command: string
  hooks_enabled: boolean
}

export interface HookStatus {
  claude: AgentHookStatus
  codex: AgentHookStatus
}

export interface HttpServerStatus {
  healthy: boolean
  retry_count: number
  last_error: string | null
}

export interface HookInstallReport {
  claude: AgentHookInstallReport
  codex: AgentHookInstallReport
}

export interface AgentHookInstallReport {
  agent: AgentName
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
  agent: AgentName
  session_id: string
  cwd: string
  project: string
  started_at: string | null
  kind: string
  summary: string
  size_bytes: number
  alias: string | null
}
