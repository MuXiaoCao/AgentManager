import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useTranslation, Trans } from 'react-i18next'
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core'
import {
  SortableContext,
  verticalListSortingStrategy,
  arrayMove,
} from '@dnd-kit/sortable'
import { SessionCard } from './components/SessionCard'
import { SortableCard } from './components/SortableCard'
import { ContextMenu, type MenuItem } from './components/ContextMenu'
import { SetupBanner } from './components/SetupBanner'
import { ClaudeHistoryList } from './components/ClaudeHistoryList'
import { currentLanguage, toggleLanguage } from './i18n'
import { getTheme, toggleTheme, type Theme } from './theme'
import type { ArrangeReport, HookStatus, SessionEntry } from './types'

type Tab = 'dashboard' | 'claude-history'

const REQUIRED_EVENTS = [
  'SessionStart',
  'UserPromptSubmit',
  'Stop',
  'Notification',
  'SessionEnd',
]

export default function App() {
  const { t, i18n } = useTranslation()
  const [sessions, setSessions] = useState<SessionEntry[]>([])
  const [hookStatus, setHookStatus] = useState<HookStatus | null>(null)
  const [toast, setToast] = useState<string | null>(null)
  const [menu, setMenu] = useState<{
    x: number
    y: number
    items: MenuItem[]
  } | null>(null)
  const [lang, setLang] = useState(currentLanguage())
  const [theme, setThemeState] = useState<Theme>(getTheme())
  const [tab, setTab] = useState<Tab>('dashboard')
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const toastTimer = useRef<number | null>(null)

  const showToast = useCallback((text: string) => {
    setToast(text)
    if (toastTimer.current) window.clearTimeout(toastTimer.current)
    toastTimer.current = window.setTimeout(() => setToast(null), 2500)
  }, [])

  const refreshSessions = useCallback(async () => {
    try {
      setSessions(await invoke<SessionEntry[]>('get_sessions'))
    } catch (err) {
      console.error('get_sessions failed', err)
    }
  }, [])

  const refreshHookStatus = useCallback(async () => {
    try {
      setHookStatus(await invoke<HookStatus>('check_hook_config'))
    } catch (err) {
      console.error('check_hook_config failed', err)
    }
  }, [])

  useEffect(() => {
    refreshSessions()
    refreshHookStatus()
    let unlisten: UnlistenFn | undefined
    listen<SessionEntry>('session-updated', () => refreshSessions())
      .then((fn) => { unlisten = fn })
      .catch(console.error)
    return () => { unlisten?.() }
  }, [refreshSessions, refreshHookStatus])

  useEffect(() => {
    const onChange = (next: string) =>
      setLang(next.startsWith('zh') ? 'zh' : 'en')
    i18n.on('languageChanged', onChange)
    return () => { i18n.off('languageChanged', onChange) }
  }, [i18n])

  const hookInstalled = useMemo(() => {
    if (!hookStatus) return true
    return (
      hookStatus.script_installed &&
      REQUIRED_EVENTS.every((e) => hookStatus.installed_events.includes(e))
    )
  }, [hookStatus])

  // Split into active vs ended (history).
  const activeSessions = useMemo(
    () => sessions.filter((s) => s.last_event !== 'sessionend'),
    [sessions]
  )
  const historySessions = useMemo(
    () => sessions.filter((s) => s.last_event === 'sessionend'),
    [sessions]
  )

  // ─── actions ──────────────────────────────────────────────────────

  const handleInstallHook = useCallback(async () => {
    try {
      await invoke('install_claude_hook')
      showToast(t('toast.installed'))
      refreshHookStatus()
    } catch (err) {
      showToast(t('toast.installFailed', { err: String(err) }))
    }
  }, [refreshHookStatus, showToast, t])

  const handleCommitRename = useCallback(
    async (sessionId: string, alias: string | null) => {
      setRenamingId(null)
      try {
        await invoke('rename_session', { sessionId, alias })
      } catch (err) {
        showToast(String(err))
      }
      refreshSessions()
    },
    [refreshSessions, showToast]
  )

  const handleCancelRename = useCallback(() => setRenamingId(null), [])

  const [flashingId, setFlashingId] = useState<string | null>(null)
  const jumpingRef = useRef(false)

  const doJump = useCallback(
    async (sessionId: string, pulse: boolean) => {
      if (jumpingRef.current) return
      jumpingRef.current = true
      setSelectedId(sessionId)
      setFlashingId(sessionId)
      setTimeout(() => setFlashingId(null), 450)
      try {
        await invoke('jump_to_iterm', { sessionId, pulse })
      } catch (err) {
        showToast(t('toast.jumpFailed', { err: String(err) }))
      } finally {
        jumpingRef.current = false
      }
    },
    [showToast, t]
  )

  const handleReopen = useCallback(
    async (sessionId: string) => {
      setSelectedId(sessionId)
      try {
        await invoke('reopen_session', { sessionId })
        showToast(t('toast.reopened'))
      } catch (err) {
        showToast(t('toast.reopenFailed', { err: String(err) }))
      }
    },
    [showToast, t]
  )

  const handleClearNotifications = useCallback(
    async (sessionId: string) => {
      await invoke('clear_notifications', { sessionId })
      refreshSessions()
    },
    [refreshSessions]
  )

  const handleDismiss = useCallback(
    async (sessionId: string) => {
      await invoke('dismiss_session', { sessionId })
      refreshSessions()
    },
    [refreshSessions]
  )

  const handleDelete = useCallback(
    async (sessionId: string) => {
      await invoke('delete_session', { sessionId })
      refreshSessions()
    },
    [refreshSessions]
  )

  // ─── dnd-kit sortable ──────────────────────────────────────────

  const sensors = useSensors(
    useSensor(PointerSensor, {
      // 8px distance before drag starts, so clicks still work for jump.
      activationConstraint: { distance: 8 },
    })
  )

  const handleSortEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event
      if (!over || active.id === over.id) return
      const oldIdx = activeSessions.findIndex((s) => s.session_id === active.id)
      const newIdx = activeSessions.findIndex((s) => s.session_id === over.id)
      if (oldIdx === -1 || newIdx === -1) return
      const reordered = arrayMove(activeSessions, oldIdx, newIdx)
      const order = reordered.map((s) => s.session_id)
      await invoke('reorder_sessions', { order })
      refreshSessions()
    },
    [activeSessions, refreshSessions]
  )

  const handleClearHistory = useCallback(async () => {
    await invoke('clear_history')
    refreshSessions()
    showToast(t('toast.historyCleared'))
  }, [refreshSessions, showToast, t])

  const handleArrangeAll = useCallback(async () => {
    try {
      const report = await invoke<ArrangeReport>('arrange_iterm_windows')
      const key =
        report.skipped > 0 ? 'toast.arrangedWithSkipped' : 'toast.arranged'
      showToast(
        t(key, {
          count: report.arranged,
          cols: report.cols,
          rows: report.rows,
          skipped: report.skipped,
        })
      )
    } catch (err) {
      showToast(t('toast.arrangeFailed', { err: String(err) }))
    }
  }, [showToast, t])

  // ─── context menus ────────────────────────────────────────────────

  const buildActiveMenu = useCallback(
    (entry: SessionEntry): MenuItem[] => [
      {
        id: 'rename',
        label: t('menu.rename'),
        onSelect: () => setRenamingId(entry.session_id),
      },
      {
        id: 'jump',
        label: t('menu.jump'),
        onSelect: () => doJump(entry.session_id, false),
        disabled:
          !entry.iterm_session_id || entry.iterm_session_id === 'unknown',
      },
      {
        id: 'clearNotif',
        label: t('menu.clearNotifications'),
        onSelect: () => handleClearNotifications(entry.session_id),
        disabled: entry.notification_count === 0,
      },
      { id: 'sep', label: '', separator: true, onSelect: () => {} },
      {
        id: 'arrange',
        label: t('menu.arrangeAll'),
        onSelect: () => handleArrangeAll(),
      },
      { id: 'sep2', label: '', separator: true, onSelect: () => {} },
      {
        id: 'dismiss',
        label: t('menu.dismiss'),
        onSelect: () => handleDismiss(entry.session_id),
        danger: true,
      },
    ],
    [doJump, handleClearNotifications, handleArrangeAll, handleDismiss, t]
  )

  const buildHistoryMenu = useCallback(
    (entry: SessionEntry): MenuItem[] => [
      {
        id: 'reopen',
        label: t('menu.reopen'),
        onSelect: () => handleReopen(entry.session_id),
      },
      {
        id: 'rename',
        label: t('menu.rename'),
        onSelect: () => setRenamingId(entry.session_id),
      },
      { id: 'sep', label: '', separator: true, onSelect: () => {} },
      {
        id: 'delete',
        label: t('menu.deleteHistory'),
        onSelect: () => handleDelete(entry.session_id),
        danger: true,
      },
    ],
    [handleReopen, handleDelete, t]
  )

  const openMenu = useCallback(
    (entry: SessionEntry, ev: React.MouseEvent) => {
      ev.preventDefault()
      setSelectedId(entry.session_id)
      const isEnded = entry.last_event === 'sessionend'
      const items = isEnded
        ? buildHistoryMenu(entry)
        : buildActiveMenu(entry)
      setMenu({ x: ev.clientX, y: ev.clientY, items })
    },
    [buildActiveMenu, buildHistoryMenu]
  )

  const closeMenu = useCallback(() => setMenu(null), [])

  // Distinguish single-click (jump, no iTerm pulse) from double-click
  // (jump + iTerm window pulse). 250ms delay on single-click to detect.
  const clickTimer = useRef<number | null>(null)

  const handleCardSingleClick = useCallback(
    (entry: SessionEntry) => {
      if (entry.last_event === 'sessionend') {
        handleReopen(entry.session_id)
        return
      }
      if (clickTimer.current) {
        // Second click arrived → double-click: jump WITH pulse
        clearTimeout(clickTimer.current)
        clickTimer.current = null
        doJump(entry.session_id, true)
      } else {
        // First click → wait to see if double-click follows
        const sid = entry.session_id
        clickTimer.current = window.setTimeout(() => {
          clickTimer.current = null
          doJump(sid, false) // single click: no iTerm pulse
        }, 250)
      }
    },
    [doJump, handleReopen]
  )

  const handleToggleLang = useCallback(() => toggleLanguage(), [])
  const handleToggleTheme = useCallback(() => {
    setThemeState(toggleTheme())
  }, [])

  const handleClaudeHistoryReopen = useCallback(
    async (sessionId: string, cwd: string) => {
      try {
        await invoke('reopen_session', { sessionId, cwd })
        showToast(t('toast.reopened'))
      } catch (err) {
        showToast(t('toast.reopenFailed', { err: String(err) }))
      }
    },
    [showToast, t]
  )

  // ─── render ───────────────────────────────────────────────────────

  const renderHistoryCard = (s: SessionEntry) => (
    <SessionCard
      key={s.session_id}
      entry={s}
      isRenaming={renamingId === s.session_id}
      isSelected={selectedId === s.session_id}
      isFlashing={flashingId === s.session_id}
      onClick={() => handleCardSingleClick(s)}
      onContextMenu={(ev) => openMenu(s, ev)}
      onDoubleClick={() => handleCardSingleClick(s)}
      onCommitRename={(alias) => handleCommitRename(s.session_id, alias)}
      onCancelRename={handleCancelRename}
      onClose={() => handleDelete(s.session_id)}
    />
  )

  return (
    <div className="app">
      <header className="app__header">
        <div className="app__tabs">
          <button
            className={`app__tab ${tab === 'dashboard' ? 'app__tab--active' : ''}`}
            onClick={() => setTab('dashboard')}
          >
            {t('tabs.dashboard')}
          </button>
          <button
            className={`app__tab ${tab === 'claude-history' ? 'app__tab--active' : ''}`}
            onClick={() => setTab('claude-history')}
          >
            {t('tabs.claudeHistory')}
          </button>
        </div>
        <div className="app__header-actions">
          {tab === 'dashboard' && (
            <button
              className="toolbar-btn"
              onClick={handleArrangeAll}
              title={t('app.arrangeButtonTitle')}
            >
              {t('app.arrangeButton')}
            </button>
          )}
          <button
            className="toolbar-btn toolbar-btn--theme"
            onClick={handleToggleTheme}
            title={theme === 'dark' ? 'Switch to light' : 'Switch to dark'}
          >
            {theme === 'dark' ? '☀️' : '🌙'}
          </button>
          <button
            className="toolbar-btn toolbar-btn--lang"
            onClick={handleToggleLang}
            title={t(
              lang === 'zh' ? 'language.toggleToEn' : 'language.toggleToZh'
            )}
          >
            🌐 {lang === 'zh' ? 'EN' : '中'}
          </button>
        </div>
      </header>

      {tab === 'dashboard' && !hookInstalled && hookStatus && (
        <SetupBanner status={hookStatus} onInstall={handleInstallHook} />
      )}

      {tab === 'dashboard' && (
        <main className="app__main">
          {sessions.length === 0 ? (
            <div className="empty">
              <p>{t('empty.title')}</p>
              <p className="empty__hint">
                <Trans i18nKey="empty.hint">
                  Start a <code>claude</code> session in iTerm and it will
                  appear here.
                </Trans>
              </p>
            </div>
          ) : (
            <>
              {activeSessions.length > 0 && (
                <DndContext
                  sensors={sensors}
                  collisionDetection={closestCenter}
                  onDragEnd={handleSortEnd}
                >
                  <SortableContext
                    items={activeSessions.map((s) => s.session_id)}
                    strategy={verticalListSortingStrategy}
                  >
                    <section>
                      {activeSessions.map((s) => (
                        <SortableCard
                          key={s.session_id}
                          entry={s}
                          isRenaming={renamingId === s.session_id}
                          isSelected={selectedId === s.session_id}
                          isFlashing={flashingId === s.session_id}
                          onClick={() => handleCardSingleClick(s)}
                          onContextMenu={(ev) => openMenu(s, ev)}
                          onDoubleClick={() => handleCardSingleClick(s)}
                          onCommitRename={(alias) =>
                            handleCommitRename(s.session_id, alias)
                          }
                          onCancelRename={handleCancelRename}
                          onClose={() => handleDismiss(s.session_id)}
                        />
                      ))}
                    </section>
                  </SortableContext>
                </DndContext>
              )}
              {historySessions.length > 0 && (
                <section>
                  <div className="section-header">
                    <h2 className="section-title">{t('history.title')}</h2>
                    <button
                      className="toolbar-btn toolbar-btn--sm toolbar-btn--danger"
                      onClick={handleClearHistory}
                    >
                      {t('history.clearAll')}
                    </button>
                  </div>
                  {historySessions.map(renderHistoryCard)}
                </section>
              )}
            </>
          )}
        </main>
      )}

      {tab === 'claude-history' && (
        <main className="app__main">
          <ClaudeHistoryList
            onReopen={handleClaudeHistoryReopen}
            showToast={showToast}
          />
        </main>
      )}

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          items={menu.items}
          onClose={closeMenu}
        />
      )}
      {toast && <div className="toast">{toast}</div>}
    </div>
  )
}
