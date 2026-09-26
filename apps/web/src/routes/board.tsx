import { createFileRoute } from '@tanstack/react-router'
import { BoardPage } from '@/components/board-page'

export interface BoardSearch {
  q?: string
  projectId?: string
  agent?: string
  status?: string
  taskId?: string
}

export const Route = createFileRoute('/board')({
  validateSearch: (search: Record<string, unknown>): BoardSearch => ({
    q: typeof search.q === 'string' ? search.q : undefined,
    projectId: typeof search.projectId === 'string' ? search.projectId : undefined,
    agent: typeof search.agent === 'string' ? search.agent : undefined,
    status: typeof search.status === 'string' ? search.status : undefined,
    taskId: typeof search.taskId === 'string' ? search.taskId : undefined,
  }),
  component: BoardPage,
})
