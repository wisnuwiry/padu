import type { AgentSession, Project, ReviewDiffSource } from '@padu/client'
import type { Dispatch, SetStateAction } from 'react'
import type { BackgroundWorkKey } from '@/lib/runtime-context'

export type PanelSurface = 'files' | 'file' | 'changes' | 'terminal' | 'backgroundWork'

export interface PanelTab {
  id: string
  surface: PanelSurface
  terminalId?: string
  title?: string
  selectedFile?: string | null
  dirty?: boolean
  backgroundWorkKey?: BackgroundWorkKey
}

export interface RightPanelHandle {
  activateTab: (tabId: string) => void
  closeTab: (tabId: string) => void
  openSurface: (
    surface: PanelSurface,
    diffSource?: ReviewDiffSource,
    backgroundWorkKey?: BackgroundWorkKey | null,
    file?: string | null,
  ) => void
  cycleTabs: (direction: 1 | -1) => void
}

export interface RightPanelProps {
  active: boolean
  open: boolean
  panelWidth: number
  session: AgentSession | null
  project?: Project
  requestedSurface: PanelSurface
  requestedDiffSource: ReviewDiffSource
  requestedBackgroundWorkKey: BackgroundWorkKey | null
  requestedFile: string | null
  requestSignal: number
  sidebarWidth: number
  sidebarVisible?: boolean
  onToggleSidebar?: () => void
  onOpenChange: (open: boolean) => void
  onPanelWidthChange: Dispatch<SetStateAction<number>>
  expanded?: boolean
  showConversation?: boolean
  onExpandedChange?: (expanded: boolean) => void
  onShowConversationChange?: (show: boolean) => void
  onExpandableChange?: (expandable: boolean) => void
  onTabsReport?: (tabs: PanelTab[], activeId: string | null) => void
  onAddToChat?: (path: string, isDir?: boolean) => void
  onFindFile?: () => void
}
