import { useCallback, useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { useTranslation } from 'react-i18next'
import { ContextMenu, type MenuItem } from './ContextMenu'
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
    if (days === 0)
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    if (days === 1) return 'yesterday'
    if (days < 7) return `${days}d ago`
    return d.toLocaleDateString([], { month: 'short', day: 'numeric' })
  } catch {
    return '—'
  }
}

function cwdShort(cwd: string): string {
  return cwd.replace(/^\/Users\/[^/]+\//, '~/').replace(/^\/home\/[^/]+\//, '~/')
}

export function ClaudeHistoryList({ onReopen, showToast }: Props) {
  const { t } = useTranslation()
  const [entries, setEntries] = useState<ClaudeHistoryEntry[]>([])
  const [loading, setLoading] = useState(true)
  const [filter, setFilter] = useState('')
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [draft, setDraft] = useState('')
  const [menu, setMenu] = useState<{
    x: number; y: number; items: MenuItem[]
  } | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      setEntries(await invoke<ClaudeHistoryEntry[]>('list_claude_sessions'))
    } catch (err) {
      showToast(String(err))
    } finally {
      setLoading(false)
    }
  }, [showToast])

  useEffect(() => { refresh() }, [refresh])

  // Focus input when entering rename mode.
  useEffect(() => {
    if (renamingId) {
      window.requestAnimationFrame(() => {
        inputRef.current?.focus()
        inputRef.current?.select()
      })
    }
  }, [renamingId])

  const commitRename = useCallback(async () => {
    if (!renamingId) return
    const trimmed = draft.trim()
    const alias = trimmed.length > 0 ? trimmed : null
    try {
      await invoke('rename_session', { sessionId: renamingId, alias })
    } catch (err) {
      showToast(String(err))
    }
    setRenamingId(null)
    refresh()
  }, [renamingId, draft, showToast, refresh])

  const cancelRename = useCallback(() => setRenamingId(null), [])

  const startRename = useCallback((e: ClaudeHistoryEntry) => {
    setRenamingId(e.session_id)
    setDraft(e.alias ?? cwdShort(e.cwd || e.project))
  }, [])

  const openMenu = useCallback(
    (e: ClaudeHistoryEntry, ev: React.MouseEvent) => {
      ev.preventDefault()
      ev.stopPropagation()
      const items: MenuItem[] = [
        {
          id: 'rename',
          label: t('menu.rename'),
          onSelect: () => startRename(e),
        },
        {
          id: 'reopen',
          label: t('menu.reopen'),
          onSelect: () => onReopen(e.session_id, e.cwd || e.project),
        },
      ]
      setMenu({ x: ev.clientX, y: ev.clientY, items })
    },
    [t, startRename, onReopen]
  )

  const filtered = filter.trim()
    ? entries.filter((e) => {
        const q = filter.toLowerCase()
        return (
          (e.alias ?? '').toLowerCase().includes(q) ||
          e.cwd.toLowerCase().includes(q) ||
          e.summary.toLowerCase().includes(q) ||
          e.session_id.toLowerCase().includes(q) ||
          e.project.toLowerCase().includes(q)
        )
      })
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
        <button
          className="toolbar-btn"
          onClick={refresh}
          title={t('claudeHistory.refresh')}
        >
          ↻
        </button>
      </div>

      {loading ? (
        <div className="empty">
          <p>{t('claudeHistory.loading')}</p>
        </div>
      ) : filtered.length === 0 ? (
        <div className="empty">
          <p>{filter ? t('claudeHistory.noMatch') : t('claudeHistory.empty')}</p>
        </div>
      ) : (
        <div className="history-list__items">
          {filtered.map((e) => {
            const isRenaming = renamingId === e.session_id
            const displayTitle = e.alias ?? cwdShort(e.cwd || e.project)

            return (
              <article
                key={e.session_id}
                className="hcard"
                onClick={() => {
                  if (!isRenaming) onReopen(e.session_id, e.cwd || e.project)
                }}
                onContextMenu={(ev) => openMenu(e, ev)}
                title={t('claudeHistory.clickToResume')}
              >
                <div className="hcard__top">
                  {isRenaming ? (
                    <input
                      ref={inputRef}
                      className="hcard__rename-input"
                      value={draft}
                      onChange={(ev) => setDraft(ev.target.value)}
                      onKeyDown={(ev) => {
                        if (ev.key === 'Enter') {
                          ev.preventDefault()
                          commitRename()
                        } else if (ev.key === 'Escape') {
                          ev.preventDefault()
                          cancelRename()
                        }
                      }}
                      onBlur={commitRename}
                      onClick={(ev) => ev.stopPropagation()}
                      onContextMenu={(ev) => ev.stopPropagation()}
                    />
                  ) : (
                    <span className="hcard__project">{displayTitle}</span>
                  )}
                  <span className="hcard__time">{formatTime(e.started_at)}</span>
                </div>
                {!isRenaming && e.alias && (
                  <div className="hcard__cwd">{cwdShort(e.cwd || e.project)}</div>
                )}
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
            )
          })}
        </div>
      )}

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menu.items}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  )
}
