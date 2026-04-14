const LS_KEY = 'agent-manager:theme'

export type Theme = 'light' | 'dark'

export function getTheme(): Theme {
  try {
    const stored = localStorage.getItem(LS_KEY)
    if (stored === 'light' || stored === 'dark') return stored
  } catch { /* ignore */ }
  // Default: follow system preference
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

export function setTheme(theme: Theme) {
  document.documentElement.setAttribute('data-theme', theme)
  try { localStorage.setItem(LS_KEY, theme) } catch { /* ignore */ }
}

export function toggleTheme(): Theme {
  const next: Theme = getTheme() === 'dark' ? 'light' : 'dark'
  setTheme(next)
  return next
}

// Apply on load
setTheme(getTheme())
