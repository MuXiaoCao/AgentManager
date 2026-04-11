import { useTranslation } from 'react-i18next'
import type { HookStatus } from '../types'

interface Props {
  status: HookStatus
  onInstall: () => void
}

export function SetupBanner({ status, onInstall }: Props) {
  const { t } = useTranslation()
  const missing = ['SessionStart', 'Stop', 'SessionEnd'].filter(
    (e) => !status.installed_events.includes(e)
  )
  return (
    <div className="setup-banner">
      <div className="setup-banner__text">
        <strong>{t('banner.notInstalled')}</strong>{' '}
        {status.script_installed ? (
          <span>{t('banner.missingEvents', { events: missing.join(', ') })}</span>
        ) : (
          <span>{t('banner.scriptMissing')}</span>
        )}
      </div>
      <button className="setup-banner__btn" onClick={onInstall}>
        {t('banner.install')}
      </button>
    </div>
  )
}
