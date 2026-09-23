---
name: mobile-ui
description: >-
  Build or review UI in apps/mobile: one-product design tokens, adaptive
  phone / flip-fold / tablet layouts, and the desktop-consistent hierarchy.
---

# Mobile UI Skill

`apps/mobile/` is the third surface of **one product**, not a separate app.
A screen must read as the same product as `apps/desktop/` and `apps/web/`, at
every window size a phone, flip phone, foldable, or tablet can produce.

Three pillars. All three are required — none is a nice-to-have:

| Pillar | Rule | Detail |
| :--- | :--- | :--- |
| **Tokens** | Colors, spacing, and radii come from `@/constants/theme`. No literal hex, no magic pixel values. | [design-tokens.md](./references/design-tokens.md) |
| **Adaptive** | Layout answers to **window width**, never to device model, hinge, or orientation. | [responsive-layout.md](./references/responsive-layout.md) |
| **Parity** | Same hierarchy and the same semantic color roles as desktop/web. | [design-tokens.md](./references/design-tokens.md#desktop--web-mapping) |

## When to use

- Any change under `apps/mobile/src/`.
- Porting a desktop/web UI change to mobile (see `protocol-parity-sync` for
  wire-level work).
- Reviewing a mobile diff before a PR.

## Step 0 — Ground yourself before writing

1. Find the desktop/web equivalent of the screen. The hierarchy must land the
   same way on mobile — read it, don't invent it.
2. Read the tokens you will use: `apps/mobile/src/constants/theme.ts`
   (generated) and [design-tokens.md](./references/design-tokens.md).
3. Decide the size classes the screen must survive:
   [responsive-layout.md](./references/responsive-layout.md) lists the required
   matrix (compact phone → cover screen → unfolded fold → tablet).

## Step 1 — Tokens, not literals

- `useTheme()` from `@/hooks/use-theme` for every color. Never
  `Colors[...]` directly in a screen, never a hex/rgba literal.
- `Spacing.*` and `Radius.*` for every gap, padding, and corner.
- Reuse an established font size; do not add an 18th size.
- `constants/theme.ts` is **generated** — edit `themes.json` and run
  `bun run theme:generate`, then `bun run theme:check`.

## Step 2 — Adaptive by width

- Read size with `useWindowDimensions()`. Never `Dimensions.get('screen')` —
  it ignores fold state, Android split-screen, and iPad Slide Over.
- Branch on **width only**. Never on `Platform.isPad`, device name, hinge
  angle, or orientation.
- Cap reading content at `MaxContentWidth` (800) and center it; never let
  prose span a tablet's full width.
- Respect safe-area insets via `useSafeAreaInsets()`; `useScreenHeaderInset()`
  for content that scrolls under the floating header.

## Step 3 — Clean hierarchy

- One primary action per screen; secondary actions behind the header cluster
  or a sheet.
- Mirror the transcript's established role treatments: user bubble on
  `raised` and right-aligned, assistant full-width with no bubble, system
  centered pill on `overlay`.
- Keep chrome floating and content beneath it (`ScreenHeader`, `GlassSurface`)
  rather than adding stacked bars.
- Every color carrying meaning is paired with an icon or text label.
- Every `Pressable` has `accessibilityRole`, an `accessibilityLabel`, and a
  ≥ 44pt hit target (`hitSlop` where the glyph is smaller).
- Honor reduce motion: gate decorative animation on
  `useReducedMotion()` from `react-native-reanimated`.
- Respect OS font scaling: never `allowFontScaling={false}`; use `minHeight`,
  not fixed `height`, around text.

## Step 4 — Verify

```bash
bun run mobile:typecheck
bun run mobile:test
bun run theme:check          # only if themes.json or tokens changed
```

Visual validation on a device/simulator happens **only when the user asks**
(see `AGENTS.md` → Development runtime). A passing typecheck is not evidence
that a layout is clean at 320pt or on a tablet.

## Review checklist

- [ ] No literal hex/rgba, no magic spacing or radius values.
- [ ] `useTheme()` used; both light and dark read correctly.
- [ ] No width logic beyond `useWindowDimensions()`; no device/orientation
      branching.
- [ ] Usable at 320pt wide and at 200% font scale: nothing clips, truncates
      gracefully, actions stay reachable.
- [ ] Wide window (≥ 1024): content centered and capped, not stretched.
- [ ] Safe-area insets applied; content clears the floating header.
- [ ] Hierarchy matches the desktop/web equivalent; semantic color roles match.
- [ ] Accessibility role/label on every control; ≥ 44pt targets.
- [ ] `bun run mobile:typecheck` and `bun run mobile:test` pass.

## Related

- `.agents/skills/protocol-parity-sync/SKILL.md` — when the change touches the
  wire protocol or shared `@padu/client` state.
- `.agents/skills/pre-pr-review/SKILL.md` — diff audit before a PR.
