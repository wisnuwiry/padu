/**
 * The light/dark mode the app should render.
 *
 * Replaces React Native's `useColorScheme` so a user's appearance preference
 * (System / Light / Dark) reaches every consumer — `useTheme()` and the few
 * surfaces that need the scheme itself. The OS only decides the result while
 * the preference is `system`; see `AppearanceProvider`.
 */
export { useAppColorScheme as useColorScheme } from '@/lib/appearance-context';
