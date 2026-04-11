import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { useTranslation, Trans } from 'react-i18next'
import { SessionCard } from './components/SessionCard'
import { ContextMenu, type MenuItem } from './components/ContextMenu'
import { SetupBanner } from './components/SetupBanner'
import { currentLanguage, toggleLanguage } from './i18n'
import type { ArrangeReport, HookStatus, SessionEntry } from './types'

const REQUIRED_EVENTS = ['SessionStart', 'Stop', 'SessionEnd']

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
  const [renamingId, setRenamingId] = useState<string | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
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

  useEffect(() => {
    const onChange = (next: string) => setLang(next.startsWith('zh') ? 'zh' : 'en')
    i18n.on('languageChanged', onChange)
    return () => {
      i18n.off('languageChanged', onChange)
    }
  }, [i18n])

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

  const handleCancelRename = useCallback(() => {
    setRenamingId(null)
  }, [])

  const handleDismiss = useCallback(
    async (sessionId: string) => {
      await invoke('dismiss_session', { sessionId })
      refreshSessions()
    },
    [refreshSessions]
  )

  const handleJump = useCallback(
    async (sessionId: string) => {
      setSelectedId(sessionId)
      try {
        await invoke('jump_to_iterm', { sessionId })
      } catch (err) {
        showToast(t('toast.jumpFailed', { err: String(err) }))
      }
    },
    [showToast, t]
  )

  const handleArrangeAll = useCallback(async () => {
    try {
      const report = await invoke<ArrangeReport>('arrange_iterm_windows')
      const key = report.skipped > 0 ? 'toast.arrangedWithSkipped' : 'toast.arranged'
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

  const buildMenu = useCallback(
    (entry: SessionEntry): MenuItem[] => [
      {
        id: 'rename',
        label: t('menu.rename'),
        onSelect: () => setRenamingId(entry.session_id),
      },
      {
        id: 'jump',
        label: t('menu.jump'),
        onSelect: () => handleJump(entry.session_id),
        disabled:
          !entry.iterm_session_id || entry.iterm_session_id === 'unknown',
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
    [handleJump, handleArrangeAll, handleDismiss, t]
  )

  const openMenu = useCallback(
    (entry: SessionEntry, ev: React.MouseEvent) => {
      ev.preventDefault()
      setSelectedId(entry.session_id)
      setMenu({ x: ev.clientX, y: ev.clientY, items: buildMenu(entry) })
    },
    [buildMenu]
  )

  const handleSelect = useCallback((sessionId: string) => {
    setSelectedId(sessionId)
  }, [])

  const closeMenu = useCallback(() => setMenu(null), [])

  const handleToggleLang = useCallback(() => {
    toggleLanguage()
  }, [])

  return (
    <div className="app">
      <header className="app__header">
        <h1>{t('app.title')}</h1>
        <div className="app__header-actions">
          <button
            className="toolbar-btn"
            onClick={handleArrangeAll}
            title={t('app.arrangeButtonTitle')}
          >
            {t('app.arrangeButton')}
          </button>
          <button
            className="toolbar-btn toolbar-btn--lang"
            onClick={handleToggleLang}
            title={t(lang === 'zh' ? 'language.toggleToEn' : 'language.toggleToZh')}
            aria-label={t(lang === 'zh' ? 'language.toggleToEn' : 'language.toggleToZh')}
          >
            🌐 {lang === 'zh' ? 'EN' : '中'}
          </button>
        </div>
      </header>

      {!hookInstalled && hookStatus && (
        <SetupBanner status={hookStatus} onInstall={handleInstallHook} />
      )}

      <main className="app__main">
        {sessions.length === 0 ? (
          <div className="empty">
            <p>{t('empty.title')}</p>
            <p className="empty__hint">
              <Trans i18nKey="empty.hint">
                Start a <code>claude</code> session in iTerm and it will appear here.
              </Trans>
            </p>
          </div>
        ) : (
          sessions.map((s) => (
            <SessionCard
              key={s.session_id}
              entry={s}
              isRenaming={renamingId === s.session_id}
              isSelected={selectedId === s.session_id}
              onClick={() => handleSelect(s.session_id)}
              onContextMenu={(ev) => openMenu(s, ev)}
              onDoubleClick={() => handleJump(s.session_id)}
              onCommitRename={(alias) => handleCommitRename(s.session_id, alias)}
              onCancelRename={handleCancelRename}
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
