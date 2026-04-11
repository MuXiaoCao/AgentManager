import type { HookStatus } from '../types'

interface Props {
  status: HookStatus
  onInstall: () => void
}

export function SetupBanner({ status, onInstall }: Props) {
  const missing = ['SessionStart', 'Stop', 'SessionEnd'].filter(
    (e) => !status.installed_events.includes(e)
  )
  return (
    <div className="setup-banner">
      <div className="setup-banner__text">
        <strong>Claude hook not fully installed.</strong>
        {status.script_installed ? (
          <span> Missing events: {missing.join(', ')}</span>
        ) : (
          <span> Hook script and settings.json entries are missing.</span>
        )}
      </div>
      <button className="setup-banner__btn" onClick={onInstall}>
        Install hook
      </button>
    </div>
  )
}
