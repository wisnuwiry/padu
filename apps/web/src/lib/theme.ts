import type { ThemeTypes } from '@pierre/diffs'
import { useEffect, useState } from 'react'

/// Resolves the active Diffs theme from Padu's `.dark`/`.light` classes,
/// reacting to class changes and OS-level appearance switches.
export function useResolvedTheme(): ThemeTypes {
  const [theme, setTheme] = useState<ThemeTypes>(() => resolvedTheme())

  useEffect(() => {
    const update = () => setTheme(resolvedTheme())
    const observer = new MutationObserver(update)
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    media.addEventListener('change', update)
    update()
    return () => {
      observer.disconnect()
      media.removeEventListener('change', update)
    }
  }, [])

  return theme
}

export function resolvedTheme(): ThemeTypes {
  if (typeof document === 'undefined') return 'system'
  if (document.documentElement.classList.contains('dark')) return 'dark'
  return 'light'
}