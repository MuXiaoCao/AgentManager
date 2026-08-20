import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useTranslation } from 'react-i18next'
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
import type {
  AgentName,
  ArrangeReport,
  HookStatus,
  HttpServerStatus,
  SessionEntry,
} from './types'

type Tab = 'dashboard' | 'claude-history' | 'codex-history'

const REQUIRED_EVENTS: Record<AgentName, string[]> = {
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

export default function App() {
  const { t, i18n } = useTranslation()
  const [sessions, setSessions] = useState<SessionEntry[]>([])
  const [hookStatus, setHookStatus] = useState<HookStatus | null>(null)
  const [httpStatus, setHttpStatus] = useState<HttpServerStatus | null>(null)
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
    let disposed = false
    const refresh = async () => {
      try {
        const status = await invoke<HttpServerStatus>('get_http_server_status')
        if (!disposed) setHttpStatus(status)
      } catch (err) {
        console.error('get_http_server_status failed', err)
      }
    }
    refresh()
    const timer = window.setInterval(refresh, 3000)
    return () => {
      disposed = true
      window.clearInterval(timer)
    }
  }, [])

  useEffect(() => {
    const onChange = (next: string) =>
      setLang(next.startsWith('zh') ? 'zh' : 'en')
    i18n.on('languageChanged', onChange)
    return () => { i18n.off('languageChanged', onChange) }
  }, [i18n])

  const hookInstalled = useMemo(() => {
    if (!hookStatus) return true
    return (Object.keys(REQUIRED_EVENTS) as AgentName[]).every((agent) => {
      const status = hookStatus[agent]
      return (
        status.script_installed &&
        status.settings_exists &&
        status.hooks_enabled &&
        REQUIRED_EVENTS[agent].every((e) => status.installed_events.includes(e))
      )
    })
  }, [hookStatus])

  // Split into active vs ended (history).
  const activeSessions = useMemo(
    () => sessions.filter((s) => s.last_event !== 'sessionend'),
    [sessions]
  )
  const historySessions = useMemo(
    () =>
      sessions
        .filter((s) => s.last_event === 'sessionend')
        .sort(
          (a, b) =>
            new Date(b.last_updated).getTime() -
            new Date(a.last_updated).getTime()
        ),
    [sessions]
  )

  // ─── actions ──────────────────────────────────────────────────────

  const handleInstallHook = useCallback(async () => {
    try {
      await invoke('install_agent_hooks')
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
    async (sessionId: string, cwd: string, agent: AgentName) => {
      try {
        await invoke('reopen_session', { sessionId, cwd, agent })
        await refreshSessions()
        showToast(t('toast.reopened'))
      } catch (err) {
        showToast(t('toast.reopenFailed', { err: String(err) }))
      }
    },
    [refreshSessions, showToast, t]
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

  const renderActiveCard = (s: SessionEntry) => (
    <SortableCard
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
      onClose={() => handleDismiss(s.session_id)}
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
          <button
            className={`app__tab ${tab === 'codex-history' ? 'app__tab--active' : ''}`}
            onClick={() => setTab('codex-history')}
          >
            {t('tabs.codexHistory')}
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

      {tab === 'dashboard' && httpStatus && !httpStatus.healthy && httpStatus.retry_count > 0 && (
        <div className="setup-banner setup-banner--error" role="status">
          <div className="setup-banner__text">
            <strong>{t('server.unavailable')}</strong>{' '}
            {t('server.retrying', { count: httpStatus.retry_count })}
          </div>
        </div>
      )}

      {tab === 'dashboard' && (
        <main className="app__main">
          <section>
            <div className="section-header">
              <h2 className="section-title">{t('active.title')}</h2>
            </div>
            {activeSessions.length > 0 ? (
              <DndContext
                sensors={sensors}
                collisionDetection={closestCenter}
                onDragEnd={handleSortEnd}
              >
                <SortableContext
                  items={activeSessions.map((s) => s.session_id)}
                  strategy={verticalListSortingStrategy}
                >
                  {activeSessions.map(renderActiveCard)}
                </SortableContext>
              </DndContext>
            ) : (
              <div className="section-empty">{t('active.empty')}</div>
            )}
          </section>

          <section>
            <div className="section-header">
              <h2 className="section-title">{t('history.title')}</h2>
              {historySessions.length > 0 && (
                <button
                  className="toolbar-btn toolbar-btn--sm toolbar-btn--danger"
                  onClick={handleClearHistory}
                >
                  {t('history.clearAll')}
                </button>
              )}
            </div>
            {historySessions.length > 0 ? (
              historySessions.map(renderHistoryCard)
            ) : (
              <div className="section-empty">{t('history.empty')}</div>
            )}
          </section>
        </main>
      )}

      {tab === 'claude-history' && (
        <main className="app__main">
          <ClaudeHistoryList
            agent="claude"
            onReopen={handleClaudeHistoryReopen}
            showToast={showToast}
          />
        </main>
      )}

      {tab === 'codex-history' && (
        <main className="app__main">
          <ClaudeHistoryList
            agent="codex"
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
