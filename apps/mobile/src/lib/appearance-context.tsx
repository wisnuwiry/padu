import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { useColorScheme as useSystemColorScheme } from 'react-native';

import {
  resolvedTheme,
  type ResolvedTheme,
  type ThemeChoice,
} from '@/lib/appearance';
import {
  cachedThemeChoice,
  loadThemeChoice,
  saveThemeChoice,
} from '@/lib/appearance-store';

interface AppearanceValue {
  /** What the user picked, persisted. */
  preference: ThemeChoice;
  /** What to render right now; never `system`. */
  mode: ResolvedTheme;
  setPreference: (next: ThemeChoice) => void;
}

const AppearanceContext = createContext<AppearanceValue | null>(null);

/**
 * Owns the one piece of appearance state the whole app derives from, so a
 * forced Light or Dark reaches every surface instead of only the ones that
 * happen to read the OS scheme.
 */
export function AppearanceProvider({ children }: { children: ReactNode }) {
  const systemScheme = useSystemColorScheme();
  const [preference, setPreferenceState] = useState<ThemeChoice>(
    () => cachedThemeChoice() ?? 'system',
  );

  // The stored choice is read once per launch; the splash screen covers the
  // window before it lands, so the default is not normally visible.
  useEffect(() => {
    let active = true;
    void loadThemeChoice().then((stored) => {
      // A choice made before the read finished wins over the stored one.
      if (active) {
        setPreferenceState((current) => (current === 'system' ? stored : current));
      }
    });
    return () => {
      active = false;
    };
  }, []);

  const setPreference = useCallback((next: ThemeChoice) => {
    setPreferenceState(next);
    void saveThemeChoice(next);
  }, []);

  const value = useMemo<AppearanceValue>(
    () => ({
      preference,
      mode: resolvedTheme(preference, systemScheme === 'dark'),
      setPreference,
    }),
    [preference, systemScheme, setPreference],
  );

  return (
    <AppearanceContext.Provider value={value}>
      {children}
    </AppearanceContext.Provider>
  );
}

export function useAppearance(): AppearanceValue {
  const context = useContext(AppearanceContext);
  if (!context) {
    throw new Error('useAppearance must be used inside AppearanceProvider');
  }
  return context;
}

/**
 * The light/dark mode to render. Drop-in replacement for React Native's
 * `useColorScheme`, except it honours the user's preference.
 */
export function useAppColorScheme(): ResolvedTheme {
  return useAppearance().mode;
}
