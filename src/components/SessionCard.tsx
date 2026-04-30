import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { SessionEntry } from '../types'

interface Props {
  entry: SessionEntry
  isRenaming: boolean
  isSelected: boolean
  isFlashing?: boolean
  onClick: () => void
  onContextMenu: (ev: React.MouseEvent) => void
  onDoubleClick: () => void
  onCommitRename: (newAlias: string | null) => void
  onCancelRename: () => void
  onClose?: () => void
}

type Tone = 'active' | 'idle' | 'done'

function eventTone(event: string): Tone {
  switch (event) {
    case 'sessionstart':
    case 'userpromptsubmit':
    case 'stop':
      return 'active'
    case 'notification':
      return 'idle'
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
    case 'userpromptsubmit':
      return 'card.status.working'
    case 'stop':
      return 'card.status.idle'
    case 'notification':
      return 'card.status.needsInput'
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
  if (diff < 3600)
    return t('card.time.minutesAgo', { count: Math.floor(diff / 60) })
  if (diff < 86400)
    return t('card.time.hoursAgo', { count: Math.floor(diff / 3600) })
  return t('card.time.daysAgo', { count: Math.floor(diff / 86400) })
}

export function SessionCard({
  entry,
  isRenaming,
  isSelected,
  isFlashing,
  onClick,
  onContextMenu,
  onDoubleClick,
  onCommitRename,
  onCancelRename,
  onClose,
}: Props) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  const fallbackTitle = useMemo(() => {
    const parts = entry.cwd.split('/').filter(Boolean)
    return parts[parts.length - 1] ?? entry.cwd
  }, [entry.cwd])

  const title = useMemo(() => {
    if (entry.alias && entry.alias.trim()) return entry.alias
    return fallbackTitle
  }, [entry.alias, fallbackTitle])

  useEffect(() => {
    if (isRenaming) {
      setDraft(entry.alias ?? fallbackTitle)
      window.requestAnimationFrame(() => {
        inputRef.current?.focus()
        inputRef.current?.select()
      })
    }
  }, [isRenaming, entry.alias, fallbackTitle])

  const commit = () => {
    const trimmed = draft.trim()
    onCommitRename(trimmed.length > 0 ? trimmed : null)
  }

  const tone = eventTone(entry.last_event)
  const statusText = t(eventKey(entry.last_event))
  const relTime = useRelativeTime(entry.last_updated)

  const className = [
    'card',
    `card--${tone}`,
    isSelected ? 'card--selected' : '',
    isFlashing ? 'card--flashing' : '',
  ]
    .filter(Boolean)
    .join(' ')

  const cwdShort = entry.cwd
    .replace(/^\/Users\/[^/]+\//, '~/')
    .replace(/^\/home\/[^/]+\//, '~/')

  return (
    <article
      className={className}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onDoubleClick={onDoubleClick}
      title={t('card.tooltip')}
    >
      {onClose && (
        <button
          className="card__close"
          onClick={(ev) => {
            ev.stopPropagation()
            onClose()
          }}
        >
          ×
        </button>
      )}

      {/* Row 1: agent badge + title + status */}
      <div className="card__header">
        <span className="card__agent-badge">{entry.agent}</span>
        {isRenaming ? (
          <input
            ref={inputRef}
            className="card__title-input"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault()
                commit()
              } else if (e.key === 'Escape') {
                e.preventDefault()
                onCancelRename()
              }
            }}
            onBlur={commit}
            onClick={(e) => e.stopPropagation()}
            onDoubleClick={(e) => e.stopPropagation()}
            onContextMenu={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="card__title">{title}</span>
        )}
        <span className={`card__status card__status--${tone}`}>
          {statusText}
        </span>
      </div>

      {/* Row 3: cwd path */}
      <div className="card__cwd" title={entry.cwd}>
        {cwdShort}
      </div>

      {/* Row 4: content preview (always shown for uniform card height) */}
      <div className="card__preview">
        {entry.preview || '\u00A0'}
      </div>

      {/* Row 5: footer with time + notifications */}
      <div className="card__footer">
        <span className="card__time">{relTime}</span>
        {entry.notification_count > 0 && (
          <span className="card__badge">
            {t('card.badge', { count: entry.notification_count })}
          </span>
        )}
      </div>
    </article>
  )
}
