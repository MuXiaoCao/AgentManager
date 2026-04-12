import { useCallback, useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useTranslation } from 'react-i18next'
import type { ClaudeHistoryEntry } from '../types'

interface Props {
  onReopen: (sessionId: string, cwd: string) => void
  showToast: (text: string) => void
}

function formatSize(bytes: number): string {
  if (bytes === 0) return ''
  if (bytes < 1024) return `${bytes}B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)}KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)}MB`
}

function formatTime(iso: string | null): string {
  if (!iso) return '—'
  try {
    const d = new Date(iso)
    const now = new Date()
    const diff = now.getTime() - d.getTime()
    const days = Math.floor(diff / 86400000)
    if (days === 0) {
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    }
    if (days === 1) return 'yesterday'
    if (days < 7) return `${days}d ago`
    return d.toLocaleDateString([], { month: 'short', day: 'numeric' })
  } catch {
    return '—'
  }
}

function cwdShort(cwd: string): string {
  return cwd
    .replace(/^\/Users\/[^/]+\//, '~/')
    .replace(/^\/home\/[^/]+\//, '~/')
}

export function ClaudeHistoryList({ onReopen, showToast }: Props) {
  const { t } = useTranslation()
  const [entries, setEntries] = useState<ClaudeHistoryEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState('')

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const list = await invoke<ClaudeHistoryEntry[]>('list_claude_sessions')
      setEntries(list)
    } catch (err) {
      showToast(String(err))
    } finally {
      setLoading(false)
    }
  }, [showToast])

  useEffect(() => {
    refresh()
  }, [refresh])

  const filtered = filter.trim()
    ? entries.filter(
        (e) =>
          e.cwd.toLowerCase().includes(filter.toLowerCase()) ||
          e.summary.toLowerCase().includes(filter.toLowerCase()) ||
          e.session_id.toLowerCase().includes(filter.toLowerCase()) ||
          e.project.toLowerCase().includes(filter.toLowerCase())
      )
    : entries

  return (
    <div className="history-list">
      <div className="history-list__toolbar">
        <input
          className="history-list__search"
          type="text"
          placeholder={t('claudeHistory.search')}
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
        />
        <button className="toolbar-btn" onClick={refresh} title={t('claudeHistory.refresh')}>
          ↻
        </button>
      </div>

      {loading ? (
        <div className="empty"><p>{t('claudeHistory.loading')}</p></div>
      ) : filtered.length === 0 ? (
        <div className="empty">
          <p>{filter ? t('claudeHistory.noMatch') : t('claudeHistory.empty')}</p>
        </div>
      ) : (
        <div className="history-list__items">
          {filtered.map((e) => (
            <article
              key={e.session_id}
              className="hcard"
              onClick={() => onReopen(e.session_id, e.cwd)}
              title={t('claudeHistory.clickToResume')}
            >
              <div className="hcard__top">
                <span className="hcard__project">{cwdShort(e.cwd || e.project)}</span>
                <span className="hcard__time">{formatTime(e.started_at)}</span>
              </div>
              {e.summary && (
                <div className="hcard__summary">{e.summary}</div>
              )}
              <div className="hcard__meta">
                <span className="hcard__id">{e.session_id.slice(0, 8)}…</span>
                {e.size_bytes > 0 && (
                  <span className="hcard__size">{formatSize(e.size_bytes)}</span>
                )}
              </div>
            </article>
          ))}
        </div>
      )}
    </div>
  )
}
