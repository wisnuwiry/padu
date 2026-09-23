# Design tokens

Mobile is the only client whose **entire** palette is generated, which makes it
the most literal expression of the shared contract. Treat it as the reference
reading of `themes.json`, not as a place to introduce local colors.

## Source of truth

```
themes.json
  └── scripts/generate-themes.ts        (bun run theme:generate)
        ├── packages/padu-client/src/theme.ts   canonical TS (COLOR_SCHEMES, BASE_TOKENS, resolveThemeColors)
        ├── apps/mobile/src/constants/theme.ts  ← full Colors + Spacing/Radius  (GENERATED)
        ├── apps/web/src/themes.css             ← ring + codeText only          (GENERATED)
        └── apps/desktop/src/theme_palette.rs   ← accent + codeText only        (GENERATED)
```

`apps/mobile/src/constants/theme.ts` carries the banner
`Generated from themes.json ... Do not edit directly.` Never hand-edit it —
your change is erased on the next `bun run theme:generate`.

### Changing or adding a color

1. Edit `themes.json` (`schemes.<id>.light|dark` for accents,
   `baseTokens.light|dark` for the palette).
2. `bun run theme:generate && bun run theme:check`.
3. **Also update the hand-written mirrors.** Web's other colors live in
   `apps/web/src/styles.css` and desktop's base colors in
   `apps/desktop/src/theme.rs`; both are manual copies of `baseTokens` that
   `theme:check` cannot detect drift on. Adding a token without them leaves
   the clients visually divergent.

## Colors — use the semantic role, not the name you like

Every color is reached through `useTheme()`:

```tsx
const theme = useTheme();
<Text style={{ color: theme.textSecondary }} />
```

| Role | Use for |
| :--- | :--- |
| `text` | Primary copy, titles. |
| `textSecondary` | Supporting copy, footnotes. |
| `textTertiary` | Metadata, timestamps, inactive icons, uppercase section labels. |
| `textGhost` | Placeholders, disabled text, decorative separators. |
| `background` | Screen canvas. |
| `surface` | Cards and grouped form rows. |
| `surfaceMuted` | Inset tiles inside a surface (icon wells, chips). |
| `raised` | Elements that lift off the canvas — user bubble, active rows. |
| `inset` | Wells that read as recessed (inputs, code gutters). |
| `composer` | The composer's own surface, both clients. |
| `backgroundElement` / `backgroundSelected` | List rows, selected rows. |
| `overlay` / `overlayStrong` | Transient wash: pressed rows, system pills, hover. |
| `separator` / `border` / `borderStrong` | Hairlines, control borders, emphasized edges. |
| `accent` / `accentSoft` | Brand action, brand wash. Never for status. |
| `success` / `warning` / `danger` (+ `*Soft`) | Status only — always paired with an icon or word. |
| `inverse` / `onInverse` | Filled primary buttons and their label. |
| `codeText` / `codeWash` | Inline code and code blocks. |
| `shadow` | Elevation only where a real elevation exists. |

`NativeTint` (iOS `systemBlue`) is the one deliberate platform-native color: it
is for *system* affordances — selection checkmarks, native control tint — where
matching the OS matters more than matching the palette.

Do not introduce a literal color. The single sanctioned exception in the tree is
the scrolled header backdrop in `components/screen-header.tsx`
(`#333333e3` / `#ffffffd6`), which must composite with a live `BlurView`. Keep
that literal local to that file; do not copy the pattern.

## Spacing

`Spacing` is the only rhythm. Pick from it; never write `padding: 13`.

| Token | Value | Typical use |
| :--- | :--- | :--- |
| `half` | 2 | Optical nudges, label offsets. |
| `one` | 4 | Inline gaps inside a chip or pill. |
| `two` | 8 | Gaps between related controls. |
| `three` | 16 | **Default** screen padding and row padding. |
| `four` | 24 | Section separation, roomier padding on wide windows. |
| `five` | 32 | Empty-state and modal breathing room. |
| `six` | 64 | Hero spacing; rare. |

Vertical rhythm: space groups with `four`, space siblings with `two`, separate
blocks with `three`. Do not mix a `gap` and a `margin` to express one spacing.
Hairlines are `StyleSheet.hairlineWidth`, never `1`.

## Radius

| Token | Value | Use |
| :--- | :--- | :--- |
| `tiny` | 6 | Chips, badges, code wash. |
| `small` | 8 | Small tiles, icon wells. |
| `medium` | 12 | **Default** — cards, sheets rows, form groups. |
| `large` | 18 | Large cards, dialogs. |
| `pill` | 999 | Capsules, round buttons, the header cluster. |

Nest radii monotonically: a child inside a `large` container uses `medium`, not
another `large`. A `pill` never contains a square-cornered surface.

## Type scale

There is currently real drift — 18 distinct `fontSize` values are in use
(`10.5` through `28`). New code must not add a 19th. Use this set:

| Role | Size | Weight | Examples |
| :--- | :--- | :--- | :--- |
| Display | 28 / 22 | 700 | Empty-state hero titles. |
| Screen title | 17 | 700 | `ScreenHeader` title. |
| Row / card title | 16 / 15.5 | 500–600 | Sheet rows, form field labels. |
| Body | 15 / 14 | 400 | Message copy, descriptions. |
| Secondary / meta | 13 / 12.5 | 400 | Subtitles, footnotes, descriptions. |
| Caption / uppercase label | 12 / 11.5 | 500–600 | Section labels, sheet titles. |

`lineHeight` ≈ 1.35–1.45 × size; set it explicitly on any multi-line body copy.
Prefer `numberOfLines` over clipping.

Never `allowFontScaling={false}`. The OS font scale is the mobile equivalent of
desktop's UI-font-size setting, and text must survive 200%.

## Desktop ↔ web mapping

Same role, different module — when porting a change, translate the *role*, never
the raw value:

| Role | Desktop | Web | Mobile |
| :--- | :--- | :--- | :--- |
| Canvas | `theme.canvas` (`theme.rs`) | `--background` (`styles.css`) | `theme.background` |
| Card | `theme.surface` | `--surface` | `theme.surface` |
| Lifted | `theme.raised` | `--raised` | `theme.raised` |
| Recessed | `theme.inset` | `--inset` | `theme.inset` |
| Accent | `theme.accent` (`theme_palette.rs`) | `--accent` / `--ring` | `theme.accent` |
| Reading column | `CONTENT_MAX_WIDTH = 720` | `max-w-[760px]` transcript, `max-w-[720px]` composer | `MaxContentWidth = 800`, cap lower for transcript |

Desktop has **no** spacing/radius token module — its values are inline
`px(...)` and it scales chrome text through `sp()`. Mobile is the only client
with a formal `Spacing`/`Radius` scale, so on mobile the scale wins.

Mobile intentionally drops `terminal`, `sidebar`, `sidebarBorder`,
`resizeHandle`, `gauge`, and `favorite` (no terminal, no sidebar, no resize
handles on a phone). Do not re-add them.

## Banned

- Literal hex / `rgba()` / `hsla()` outside the sanctioned header backdrop.
- Numeric padding, margin, gap, or radius where a token exists.
- A new font size, or a `fontWeight` outside `'400' | '500' | '600' | '700'`.
- `Colors[...]` read directly in a screen instead of `useTheme()`.
- Editing `constants/theme.ts` by hand.
- Status color without an accompanying icon or label.
- Fixed `height` around text (breaks at large font scales).
