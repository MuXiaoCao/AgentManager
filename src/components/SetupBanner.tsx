import { useTranslation } from 'react-i18next'
import type { AgentHookStatus, HookStatus } from '../types'

interface Props {
  status: HookStatus
  onInstall: () => void
}

const REQUIRED_EVENTS: Record<string, string[]> = {
  claude: [
    'SessionStart',
    'UserPromptSubmit',
    'Stop',
    'Notification',
    'SessionEnd',
  ],
  codex: [
    'SessionStart',
    'UserPromptSubmit',
    'PermissionRequest',
    'Stop',
    'SessionEnd',
  ],
}

function missingEvents(status: AgentHookStatus): string[] {
  const required = REQUIRED_EVENTS[status.agent] ?? []
  return required.filter((e) => !status.installed_events.includes(e))
}

export function SetupBanner({ status, onInstall }: Props) {
  const { t } = useTranslation()
  const agents = [status.claude, status.codex]
  return (
    <div className="setup-banner">
      <div className="setup-banner__text">
        <strong>{t('banner.notInstalled')}</strong>
        {agents.map((agent) => {
          const missing = missingEvents(agent)
          if (
            agent.script_installed &&
            agent.settings_exists &&
            missing.length === 0 &&
            agent.hooks_enabled
          ) {
            return null
          }
          const label = t(`agent.${agent.agent}`)
          const detail = !agent.hooks_enabled
            ? t('banner.hooksDisabled')
            : !agent.script_installed || !agent.settings_exists
              ? t('banner.scriptMissing')
              : t('banner.missingEvents', { events: missing.join(', ') })
          return (
            <span key={agent.agent}>
              {' '}
              {label}: {detail}
            </span>
          )
        })}
      </div>
      <button className="setup-banner__btn" onClick={onInstall}>
        {t('banner.install')}
      </button>
    </div>
  )
}
