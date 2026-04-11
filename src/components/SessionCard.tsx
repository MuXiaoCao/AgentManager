import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { SessionEntry } from '../types'

interface Props {
  entry: SessionEntry
  onContextMenu: (ev: React.MouseEvent) => void
  onDoubleClick: () => void
}

type Tone = 'active' | 'idle' | 'done'

function eventTone(event: string): Tone {
  switch (event) {
    case 'sessionstart':
      return 'active'
    case 'notification':
      return 'idle'
    case 'stop':
    case 'sessionend':
      return 'done'
    default:
      return 'idle'
  }
}

function eventKey(event: string): string {
  switch (event) {
    case 'sessionstart':
      return 'card.status.started'
    case 'notification':
      return 'card.status.needsInput'
    case 'stop':
      return 'card.status.stopped'
    case 'sessionend':
      return 'card.status.ended'
    default:
      return 'card.status.unknown'
  }
}

function useRelativeTime(iso: string): string {
  const { t } = useTranslation()
  const then = new Date(iso).getTime()
  const now = Date.now()
  const diff = Math.max(0, Math.round((now - then) / 1000))
  if (diff < 5) return t('card.time.justNow')
  if (diff < 60) return t('card.time.secondsAgo', { count: diff })
  if (diff < 3600) return t('card.time.minutesAgo', { count: Math.floor(diff / 60) })
  if (diff < 86400) return t('card.time.hoursAgo', { count: Math.floor(diff / 3600) })
  return t('card.time.daysAgo', { count: Math.floor(diff / 86400) })
}

export function SessionCard({ entry, onContextMenu, onDoubleClick }: Props) {
  const { t } = useTranslation()

  const title = useMemo(() => {
    if (entry.alias && entry.alias.trim()) return entry.alias
    const parts = entry.cwd.split('/').filter(Boolean)
    return parts[parts.length - 1] ?? entry.cwd
  }, [entry.alias, entry.cwd])

  const tone = eventTone(entry.last_event)
  const statusText = t(eventKey(entry.last_event))
  const relTime = useRelativeTime(entry.last_updated)

  return (
    <article
      className={`card card--${tone}`}
      onContextMenu={onContextMenu}
      onDoubleClick={onDoubleClick}
      title={t('card.tooltip')}
    >
      <div className="card__top">
        <span className="card__title">{title}</span>
        <span className={`card__status card__status--${tone}`}>{statusText}</span>
      </div>
      <div className="card__cwd" title={entry.cwd}>
        {entry.cwd}
      </div>
      <div className="card__meta">
        <span className="card__agent">{entry.agent}</span>
        <span className="card__time">{relTime}</span>
      </div>
      {entry.notification_count > 0 && (
        <div className="card__badge">
          {t('card.badge', { count: entry.notification_count })}
        </div>
      )}
    </article>
  )
}
