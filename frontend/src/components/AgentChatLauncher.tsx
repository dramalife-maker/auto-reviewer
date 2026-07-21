import { useRef, useState, type ReactNode } from 'react'

import { useDraggablePosition } from '../lib/useDraggablePosition'
import { AgentChatPanel, type AgentChatMessage } from './AgentChatPanel'
import { Button, Card } from './ui'

type AgentChatLauncherProps = {
  messages: AgentChatMessage[]
  input: string
  loading: boolean
  readOnly?: boolean
  inputDisabled?: boolean
  emptyHint: string
  placeholder: string
  titleSuffix?: string
  onInputChange: (value: string) => void
  onSend: () => void
  open?: boolean
  defaultOpen?: boolean
  onOpenChange?: (open: boolean) => void
  panelClassName?: string
}

const FAB_POSITION_KEY = 'agent-chat-fab-position'
const PANEL_POSITION_KEY = 'agent-chat-panel-position'

export function AgentChatLauncher({
  open: openProp,
  defaultOpen = false,
  onOpenChange,
  panelClassName = 'h-[min(70vh,560px)]',
  ...panelProps
}: AgentChatLauncherProps): ReactNode {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen)
  const isControlled = openProp !== undefined
  const open = isControlled ? openProp : uncontrolledOpen

  const fabRef = useRef<HTMLButtonElement>(null)
  const panelRef = useRef<HTMLDivElement>(null)
  const fabDrag = useDraggablePosition(FAB_POSITION_KEY, fabRef)
  const panelDrag = useDraggablePosition(PANEL_POSITION_KEY, panelRef)

  function setOpen(next: boolean) {
    if (!isControlled) {
      setUncontrolledOpen(next)
    }
    onOpenChange?.(next)
  }

  if (!open) {
    return (
      <Button
        ref={fabRef}
        aria-label="展開 Agent Chat"
        className="fixed z-40 !size-14 !rounded-full !p-0 shadow-none touch-none"
        style={
          fabDrag.position
            ? { right: fabDrag.position.right, bottom: fabDrag.position.bottom }
            : { right: '1.5rem', bottom: '1.5rem' }
        }
        onClick={() => {
          if (!fabDrag.wasDragged()) {
            setOpen(true)
          }
        }}
        title="展開 Agent Chat（可拖曳）"
        variant="mr"
        {...fabDrag.dragHandleProps}
      >
        <svg aria-hidden="true" className="size-6" fill="currentColor" viewBox="0 0 24 24">
          <path d="M4 4.75A2.75 2.75 0 0 1 6.75 2h10.5A2.75 2.75 0 0 1 20 4.75v8.5A2.75 2.75 0 0 1 17.25 16H9.06l-3.53 2.82A1 1 0 0 1 4 18.05V4.75ZM6.75 4A.75.75 0 0 0 6 4.75v11.2l2.03-1.62a1 1 0 0 1 .62-.2h8.6a.75.75 0 0 0 .75-.75v-8.5a.75.75 0 0 0-.75-.75H6.75Z" />
        </svg>
      </Button>
    )
  }

  return (
    <Card
      ref={panelRef}
      className="fixed z-40 flex w-[min(100vw-2rem,400px)] flex-col overflow-hidden p-5 shadow-none"
      style={
        panelDrag.position
          ? { right: panelDrag.position.right, bottom: panelDrag.position.bottom }
          : { right: '1rem', bottom: '1rem' }
      }
    >
      <AgentChatPanel
        {...panelProps}
        className={panelClassName}
        headerProps={{ className: 'cursor-move touch-none', ...panelDrag.dragHandleProps }}
        onClose={() => setOpen(false)}
      />
    </Card>
  )
}
