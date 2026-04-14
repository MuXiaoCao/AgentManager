import { useSortable } from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { SessionCard } from './SessionCard'
import type { SessionEntry } from '../types'

interface Props {
  entry: SessionEntry
  isRenaming: boolean
  isSelected: boolean
  onClick: () => void
  onContextMenu: (ev: React.MouseEvent) => void
  onDoubleClick: () => void
  onCommitRename: (newAlias: string | null) => void
  onCancelRename: () => void
}

export function SortableCard(props: Props) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: props.entry.session_id })

  const style: React.CSSProperties = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
    zIndex: isDragging ? 10 : 'auto',
    position: 'relative' as const,
  }

  return (
    <div ref={setNodeRef} style={style} {...attributes} {...listeners}>
      <SessionCard {...props} />
    </div>
  )
}
