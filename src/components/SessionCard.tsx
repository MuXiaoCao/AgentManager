import { useMemo } from 'react'
import type { SessionEntry } from '../types'

interface Props {
  entry: SessionEntry
  onContextMenu: (ev: React.MouseEvent) => void
  onDoubleClick: () => void
}

function relativeTime(iso: string): string {
  const then = new Date(iso).getTime()
  const now = Date.now()
  const diff = Math.max(0, Math.round((now - then) / 1000))
  if (diff < 5) return 'just now'
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

function eventLabel(event: string): { text: string; tone: 'active' | 'idle' | 'done' } {
  switch (event) {
    case 'sessionstart':
      return { text: 'Started', tone: 'active' }
    case 'notification':
      return { text: 'Needs input', tone: 'idle' }
    case 'stop':
      return { text: 'Stopped', tone: 'done' }
    case 'sessionend':
      return { text: 'Ended', tone: 'done' }
    default:
      return { text: event || '—', tone: 'idle' }
  }
}

export function SessionCard({ entry, onContextMenu, onDoubleClick }: Props) {
  const title = useMemo(() => {
    if (entry.alias && entry.alias.trim()) return entry.alias
    const parts = entry.cwd.split('/').filter(Boolean)
    return parts[parts.length - 1] ?? entry.cwd
  }, [entry.alias, entry.cwd])

  const { text: statusText, tone } = eventLabel(entry.last_event)

  return (
    <article
      className={`card card--${tone}`}
      onContextMenu={onContextMenu}
      onDoubleClick={onDoubleClick}
      title="Right-click for actions · double-click to jump to iTerm"
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
        <span className="card__time">{relativeTime(entry.last_updated)}</span>
      </div>
      {entry.notification_count > 0 && (
        <div className="card__badge">
          {entry.notification_count} notification{entry.notification_count === 1 ? '' : 's'}
        </div>
      )}
    </article>
  )
}
