import { createFileRoute } from '@tanstack/react-router'
import { NotesPage } from '@/components/notes-page'

export const Route = createFileRoute('/notes')({
  validateSearch: (search: Record<string, unknown>) => ({
    q: typeof search.q === 'string' ? search.q : undefined,
    projectId: typeof search.projectId === 'string' ? search.projectId : undefined,
  }),
  component: NotesPage,
})
