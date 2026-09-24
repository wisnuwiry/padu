import AsyncStorage from '@react-native-async-storage/async-storage';

import { parseThemeChoice, THEME_KEY, type ThemeChoice } from '@/lib/appearance';

/** Last known choice, so the first render can use the stored value instead of
 * flashing the default. Null until the first load finishes. */
let cached: ThemeChoice | null = null;

export function cachedThemeChoice(): ThemeChoice | null {
  return cached;
}

export async function loadThemeChoice(): Promise<ThemeChoice> {
  try {
    cached = parseThemeChoice(await AsyncStorage.getItem(THEME_KEY));
  } catch {
    // Unreadable storage is not worth failing a launch over.
    cached = 'system';
  }
  return cached;
}

export async function saveThemeChoice(choice: ThemeChoice): Promise<void> {
  cached = choice;
  try {
    await AsyncStorage.setItem(THEME_KEY, choice);
  } catch {
    // A preference that cannot be written must still apply this session.
  }
}
