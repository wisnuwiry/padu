// Width breakpoints shared by every adaptive mobile screen.
//
// Aligned to the web client's Tailwind breakpoints so "wide" means the same
// thing on both surfaces: web switches its sidebar from an off-canvas drawer
// to an inline pane exactly at `lg` (1024px).
//
// Always branch on window width from `useWindowDimensions()` — never on
// device model, `Platform.isPad`, hinge angle, or orientation.
export const Breakpoints = {
  /** `regular` starts at web `sm` (640). Below is single-column compact. */
  regular: 640,
  /** `wide` starts at web `lg` (1024). Only here are two panes allowed. */
  wide: 1024,
} as const;

/**
 * Minimum widths borrowed from the desktop client for a list→detail split,
 * so the mobile two-pane never invents tighter minima than desktop.
 */
export const PaneMinima = {
  /** Desktop `MAIN_PANEL_MIN_WIDTH` — the detail pane floor. */
  detail: 360,
  /** Desktop rail range 320–360 — the list rail lives inside it. */
  listRailMin: 320,
  listRailMax: 360,
} as const;
