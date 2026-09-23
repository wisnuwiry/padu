# Adaptive layout — phone, flip/fold, tablet

The mobile tree currently reads **no width at all**: the only
`useWindowDimensions()` calls take `height` for sheet sizing, and
`MaxContentWidth` is exported but unused. Every rule below is therefore the
contract to build toward, not a description of existing code.

## One principle

> Adapt to the **window**, never to the **device**.

Window size already encodes fold state, multitasking, and rotation. A fold
unfolded, an Android split-screen column, and an iPad Slide Over window are all
just "a different width" — and all three must re-lay-out live.

```tsx
const { width, height } = useWindowDimensions();   // correct
const { width } = Dimensions.get('screen');        // WRONG — physical panel
```

`Dimensions.get('screen')` reports the panel, so a two-pane layout would persist
on a half-open fold or inside a split-screen column, where it cannot fit.

Branch on width **only**:

```tsx
const isWide = width >= 1024;                      // correct
const isTablet = Platform.isPad;                   // WRONG
const isLandscape = orientation === 'landscape';   // WRONG
```

## Size classes

Aligned to the web client's Tailwind breakpoints, so "wide" means the same thing
in both clients. Web switches its sidebar from an off-canvas drawer to an inline
pane exactly at `lg` (1024px), and its JS uses `matchMedia('(max-width: 1023px)')`
for the same decision.

| Class | Window width | Web equivalent |
| :--- | :--- | :--- |
| `compact` | < 640 | `< sm` (640) |
| `regular` | 640 – 1023 | `sm` … `< lg` |
| `wide` | ≥ 1024 | `≥ lg` (1024) — web's inline-sidebar threshold |

`768` (web `md`) is not a class boundary; it is the point where the wider
reading measure and roomier horizontal padding begin.

### What each class allows

| Class | Layout |
| :--- | :--- |
| `compact` | Single column. `Spacing.three` (16) screen padding. Bottom sheets for pickers. Full-bleed lists with hairline separators. No multi-column. |
| `regular` | Single **centered** column. Horizontal padding `Spacing.four`–`five` (24–32). Reading column capped (see below). Rows keep the capped measure, not the window width. Still bottom sheets. |
| `wide` | Centered capped column. Two panes allowed for list→detail, mirroring web's `lg` inline sidebar. |

### Reading column

Never let prose span the window. Cap and center it:

- Transcript: **≤ 760** — matches web's `max-w-[760px]` so prose wraps the same.
- Composer / forms: **≤ 720** — matches web's `max-w-[720px]` and desktop's
  `CONTENT_MAX_WIDTH = 720`.
- Hard ceiling: `MaxContentWidth` (800) from `constants/theme.ts`.
- Desktop's `CONTENT_MAX_WIDTH = 720` and web's 760 are the reference family;
  do not exceed 800 on any surface.

### Two panes, when width allows

At `wide` (≥ 1024), a list→detail screen may show both panes. Borrow the
desktop's minima rather than inventing new ones:

- detail pane ≥ **360** (desktop `MAIN_PANEL_MIN_WIDTH`)
- list rail **320–360**
- below the sum of the minima, drop the rail and return to push navigation —
  the same progressive-shrink logic as desktop's `fitted_panel_widths`.

An iPad in **portrait** is ~820pt wide → `regular`, **not** `wide`. Expect a
single-pane tablet in portrait; two panes only in landscape and on 12.9"+ sizes.
Do not assume "tablet ⇒ two panes".

## Required matrix

Every screen must be checked against this. Rows marked ⚠ are the ones that break
naive layouts.

| Target | Window (pt) | Class | Why it matters |
| :--- | :--- | :--- | :--- |
| iPhone SE | 375 × 667 | compact | Smallest width; short height. |
| iPhone 15 Pro | 393 × 852 | compact | Baseline phone. |
| iPhone landscape | 852 × 393 | regular ⚠ | Wide **and** very short. |
| Flip cover screen | ~ 344 × 882 | compact ⚠ | The app may launch here; must be usable without unfolding. |
| Flip unfolded | ~ 412 × 882 | compact ⚠ | Unfolded is *still* compact — width decides, not "is it folded". |
| Fold unfolded | ~ 840 × 900 | regular | Inner display, one column. |
| Foldable half-open | ~ 840 × 480 | regular ⚠ | Short window; fixed heights break. |
| Tablet portrait | 820 × 1180 | regular ⚠ | Single pane despite being a tablet. |
| Tablet landscape | 1180 × 820 | wide | Two-pane eligible. |
| Tablet portrait 13" | 1024 × 1366 | wide | Two-pane eligible in portrait. |
| iPad Split View ⅓ | ~ 320 × 1180 | compact ⚠ | Narrow window on a large device. |
| Android split-screen | varies | compact/regular | Same code path as fold. |
| Any + 200% font scale | — | — | Text must not clip or overlap. |

## Fold and flip specifics

- **No hinge API exists.** React Native and Expo expose no fold-angle or hinge
  geometry API. Do not add a dependency, device-model table, or build flag to
  detect the hinge. Adapt by width; where a device reports hinge dead space it
  arrives as a safe-area inset, which `useSafeAreaInsets()` already handles.
- **Half-open is a short window, not a special case.** Never rely on a fixed
  content height. Sheets already size by fraction (`0.62`, `0.48` of window
  height) — keep that pattern; it survives a 480pt-tall window.
- **Cover screens are narrow.** At ≤ 360pt nothing may clip: no fixed widths, no
  `minWidth`, no horizontal scroll for primary content. Truncate with
  `numberOfLines`, wrap with `flexWrap`, and keep ≥ 44pt hit targets even when
  space is tight.
- **Do not lock orientation to dodge landscape.** `app.json` sets
  `"orientation": "default"` and `"supportsTablet": true` deliberately. Both
  orientations must work at every size class.

## Tablet specifics

- Widths from ~320 (Split View ⅓) to 1366 (13" landscape) are all reachable.
- A tablet is the same column with more margin — not a stretched phone. Cap the
  measure and center; do not scale up type or spacing.
- Keep native presentation: `pageSheet` and bottom sheets remain correct on
  iPad. Do not switch to a popover just because width allows, unless the
  desktop/web equivalent does.
- iPad with a hardware keyboard shows no software keyboard — `KeyboardAvoidingView`
  must not leave a phantom gap when no keyboard is present.

## Safe areas

- Always `useSafeAreaInsets()`. Android is edge-to-edge, so the bottom inset is
  real and non-zero — `safe-area-context`, not a guessed constant.
- Content that scrolls under the floating header uses `useScreenHeaderInset()`.
- Where a whole screen wraps content, use `SafeAreaView` with explicit `edges`.
- `BottomTabInset` exists but is unused legacy; prefer insets from
  `useSafeAreaInsets()`.

## Convention to establish

```ts
// apps/mobile/src/constants/layout.ts
export const Breakpoints = { regular: 640, wide: 1024 } as const;

// apps/mobile/src/hooks/use-responsive.ts
export type SizeClass = 'compact' | 'regular' | 'wide';
export function useResponsive(): {
  width: number;
  height: number;
  sizeClass: SizeClass;
  isLandscape: boolean;
};
```

Reuse `MaxContentWidth` from `constants/theme.ts` for the cap — do not add a
second width constant beside it.

## Anti-patterns

| Don't | Because |
| :--- | :--- |
| `Dimensions.get('screen')` | Ignores fold state and multitasking; mislays out on a half-open fold. |
| `Platform.isPad` / device-name checks | An iPad in Split View is 320pt; a fold unfolded is 840pt. |
| Orientation-based branching | Landscape phones are wide *and* short; width alone decides. |
| Hinge/fold-angle detection | No API exists; width + safe-area insets already cover it. |
| Fixed `height` on a container with text | Breaks on 200% font scale and on short fold windows. |
| Full-width prose on a wide window | Unreadable measure; violates desktop/web parity. |
| Fixed-width side rail with no floor | Clips on a cover screen or split view. |
| `allowFontScaling={false}` | Removes the OS accessibility setting. |

## Verify

```bash
bun run mobile:typecheck
bun run mobile:test
```

Then reason against the matrix: the smallest width (320/360), the worst aspect
ratio (landscape phone, half-open fold), and 200% font scale. Visual validation
on a device happens **only when the user asks** (`AGENTS.md` → Development
runtime).
