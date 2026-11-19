export interface SkillWarningSessionState {
  sessionId: string;
  warnings: readonly string[];
}

/**
 * Initialize warning props only when the composer moves to another session.
 * Ordinary transcript or streaming renders may pass a fresh array reference
 * for the same session; those must not clear warnings already shown.
 */
export function reconcileSkillWarningSession(
  current: SkillWarningSessionState,
  sessionId: string,
  initialWarnings: readonly string[],
): SkillWarningSessionState {
  if (current.sessionId === sessionId) return current;
  return { sessionId, warnings: [...initialWarnings] };
}
