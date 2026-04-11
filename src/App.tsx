import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { SessionCard } from './components/SessionCard'
import { ContextMenu, type MenuItem } from './components/ContextMenu'
import { SetupBanner } from './components/SetupBanner'
import type { ArrangeReport, HookStatus, SessionEntry } from './types'

const REQUIRED_EVENTS = ['SessionStart', 'Stop', 'SessionEnd']

export default function App() {
  const [sessions, setSessions] = useState<SessionEntry[]>([])
  const [hookStatus, setHookStatus] = useState<HookStatus | null>(null)
  const [toast, setToast] = useState<string | null>(null)
  const [menu, setMenu] = useState<{
    x: number
    y: number
    items: MenuItem[]
  } | null>(null)
  const toastTimer = useRef<number | null>(null)

  const showToast = useCallback((text: string) => {
    setToast(text)
    if (toastTimer.current) {
      window.clearTimeout(toastTimer.current)
    }
    toastTimer.current = window.setTimeout(() => setToast(null), 2500)
  }, [])

  const refreshSessions = useCallback(async () => {
    try {
      const list = await invoke<SessionEntry[]>('get_sessions')
      setSessions(list)
    } catch (err) {
      console.error('get_sessions failed', err)
    }
  }, [])

  const refreshHookStatus = useCallback(async () => {
    try {
      const status = await invoke<HookStatus>('check_hook_config')
      setHookStatus(status)
    } catch (err) {
      console.error('check_hook_config failed', err)
    }
  }, [])

  useEffect(() => {
    refreshSessions()
    refreshHookStatus()
    let unlisten: UnlistenFn | undefined
    listen<SessionEntry>('session-updated', () => {
      refreshSessions()
    })
      .then((fn) => {
        unlisten = fn
      })
      .catch(console.error)
    return () => {
      unlisten?.()
    }
  }, [refreshSessions, refreshHookStatus])

  const hookInstalled = useMemo(() => {
    if (!hookStatus) return true
    return (
      hookStatus.script_installed &&
      REQUIRED_EVENTS.every((e) => hookStatus.installed_events.includes(e))
    )
  }, [hookStatus])

  const handleInstallHook = useCallback(async () => {
    try {
      await invoke('install_claude_hook')
      showToast('Hook installed. Restart running Claude sessions to activate.')
      refreshHookStatus()
    } catch (err) {
      showToast(`Install failed: ${err}`)
    }
  }, [refreshHookStatus, showToast])

  const handleRename = useCallback(
    async (sessionId: string, alias: string | null) => {
      await invoke('rename_session', { sessionId, alias })
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

  const handleJump = useCallback(
    async (sessionId: string) => {
      try {
        await invoke('jump_to_iterm', { sessionId })
      } catch (err) {
        showToast(`Jump failed: ${err}`)
      }
    },
    [showToast]
  )

  const handleArrangeAll = useCallback(async () => {
    try {
      const report = await invoke<ArrangeReport>('arrange_iterm_windows')
      showToast(
        `Arranged ${report.arranged} iTerm window(s) into ${report.cols}×${report.rows}` +
          (report.skipped > 0 ? ` (${report.skipped} skipped)` : '')
      )
    } catch (err) {
      showToast(`Arrange failed: ${err}`)
    }
  }, [showToast])

  const buildMenu = useCallback(
    (entry: SessionEntry): MenuItem[] => [
      {
        id: 'rename',
        label: 'Rename…',
        onSelect: () => {
          const initial = entry.alias ?? ''
          const next = window.prompt('Rename session', initial)
          if (next === null) return
          handleRename(entry.session_id, next.trim() ? next.trim() : null)
        },
      },
      {
        id: 'jump',
        label: 'Jump to iTerm',
        onSelect: () => handleJump(entry.session_id),
        disabled:
          !entry.iterm_session_id || entry.iterm_session_id === 'unknown',
      },
      { id: 'sep', label: '', separator: true, onSelect: () => {} },
      {
        id: 'arrange',
        label: 'Arrange all iTerm windows',
        onSelect: () => handleArrangeAll(),
      },
      { id: 'sep2', label: '', separator: true, onSelect: () => {} },
      {
        id: 'dismiss',
        label: 'Dismiss',
        onSelect: () => handleDismiss(entry.session_id),
        danger: true,
      },
    ],
    [handleRename, handleJump, handleArrangeAll, handleDismiss]
  )

  const openMenu = useCallback(
    (entry: SessionEntry, ev: React.MouseEvent) => {
      ev.preventDefault()
      setMenu({ x: ev.clientX, y: ev.clientY, items: buildMenu(entry) })
    },
    [buildMenu]
  )

  const closeMenu = useCallback(() => setMenu(null), [])

  return (
    <div className="app">
      <header className="app__header">
        <h1>AgentManager</h1>
        <button
          className="toolbar-btn"
          onClick={handleArrangeAll}
          title="Arrange all iTerm windows into a grid"
        >
          ▦ Arrange
        </button>
      </header>

      {!hookInstalled && hookStatus && (
        <SetupBanner status={hookStatus} onInstall={handleInstallHook} />
      )}

      <main className="app__main">
        {sessions.length === 0 ? (
          <div className="empty">
            <p>No active Claude sessions.</p>
            <p className="empty__hint">
              Start a <code>claude</code> session in iTerm and it will appear here.
            </p>
          </div>
        ) : (
          sessions.map((s) => (
            <SessionCard
              key={s.session_id}
              entry={s}
              onContextMenu={(ev) => openMenu(s, ev)}
              onDoubleClick={() => handleJump(s.session_id)}
            />
          ))
        )}
      </main>

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
