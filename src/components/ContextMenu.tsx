import { useEffect, useRef } from 'react'

export interface MenuItem {
  id: string
  label: string
  onSelect: () => void
  disabled?: boolean
  danger?: boolean
  separator?: boolean
}

interface Props {
  x: number
  y: number
  items: MenuItem[]
  onClose: () => void
}

export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function onDown(ev: MouseEvent) {
      if (ref.current && !ref.current.contains(ev.target as Node)) {
        onClose()
      }
    }
    function onKey(ev: KeyboardEvent) {
      if (ev.key === 'Escape') onClose()
    }
    document.addEventListener('mousedown', onDown)
    document.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('mousedown', onDown)
      document.removeEventListener('keydown', onKey)
    }
  }, [onClose])

  // Keep the menu within viewport bounds.
  const style: React.CSSProperties = {
    left: Math.min(x, window.innerWidth - 220),
    top: Math.min(y, window.innerHeight - items.length * 30 - 10),
  }

  return (
    <div className="ctx" ref={ref} style={style} role="menu">
      {items.map((item) => {
        if (item.separator) {
          return <div key={item.id} className="ctx__sep" />
        }
        const cls = [
          'ctx__item',
          item.disabled ? 'ctx__item--disabled' : '',
          item.danger ? 'ctx__item--danger' : '',
        ]
          .filter(Boolean)
          .join(' ')
        return (
          <button
            key={item.id}
            type="button"
            className={cls}
            disabled={item.disabled}
            onClick={() => {
              if (item.disabled) return
              item.onSelect()
              onClose()
            }}
            role="menuitem"
          >
            {item.label}
          </button>
        )
      })}
    </div>
  )
}
