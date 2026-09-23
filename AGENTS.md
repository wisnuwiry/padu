# Padu development guidance

## Development runtime

- Assume `bun ./scripts/dev.ts` is already running and owns the current
  `Padu Debug.app` process. Source changes are rebuilt, signed, and relaunched
  automatically. Only run it yourself if not already launched.
- During normal development and UI validation, do not run
  `scripts/bundle.sh debug`, start a second watcher, or manually quit/relaunch
  `Padu Debug.app`. Quitting the app also stops the watcher.
- After an edit, wait for the watcher to finish its successful rebuild and
  validate the freshly relaunched debug app. Only start or recover the watcher
  manually when it is confirmed unavailable.
- No visual test unless requested.

## Performance

- Treat performance as a product requirement, not a follow-up. Padu is a native
  app competing with web clients, and staying smooth under a long transcript on
  a high-refresh display is the point of being native. Prefer the faster design
  when it costs nothing in clarity, and measure before assuming a cost is fine.
- Never block the UI thread with heavy work. Rendering owns it, so anything a
  frame can reach must already be in memory: no subprocess spawns, no
  filesystem walks, no network, no blocking locks, no synchronous IPC.
- Row builders and measurement paths run for every visible item on every frame.
  Treat I/O reached from `render` as a defect even when it looks cheap, is
  cached after the first hit, or only triggers for some rows — one `git`
  invocation is already several frames of budget.
- Move the work to `cx.background_executor().spawn`, store the result on the
  entity, and `cx.notify()` when it lands. Render then reads only that store,
  and a miss means "not known yet" and must degrade gracefully.
- Resolve a whole session or collection in one background pass instead of
  probing per item, and guard it with a generation counter so a result from a
  superseded pass cannot overwrite newer state.
- One-shot user actions such as a click or menu command may work synchronously
  when freshness matters more than latency; frames may not.
- Keep per-frame work proportional to what is on screen. Long collections are
  virtualized with `list()`, and a row builder must not rebuild whole-session
  state; hoist that to a cache refreshed once per frame.
- Streaming CPU is governed by two cadences — stream commits at ≤ ~8.3 Hz and
  pulse-clock ticks at ≤ ~30 Hz — and by what one frame can see. Read
  [docs/performance.md](docs/performance.md) before touching the event pump,
  the pulse clock (`apps/desktop/src/ui/motion.rs`), veils, overlay scrollbars, pane
  caching, or anything else a streaming frame reaches; it also records the
  counter-based measurement playbook that actually finds regressions.

## Accessibility

- Treat accessibility as a product requirement too. GPUI does not yet expose a
  screen-reader tree, so here it means keyboard operability, honored system
  settings, and legibility — none of which depend on that missing API, and all
  of which regress silently if left unchecked.
- Every control reachable by mouse must be reachable and operable by keyboard.
  Use `track_focus` with `tab_index`, `tab_group`, and `tab_stop`, give focus a
  visible treatment via `focus_visible`, and support the conventional keys for
  the widget (arrows, `home`/`end`, `enter`/`space`, `escape`).
- Honor the system's reduce-motion setting. `with_animation` already respects
  `App::reduce_motion`, but a direct `window.request_animation_frame` for
  decorative motion must check `cx.reduce_motion()` and skip the request.
- Never encode meaning in color, hover, or motion alone. Pair a status color
  with an icon or text, and make sure anything revealed on hover is also
  reachable by keyboard focus.
- Keep text and icons legible against their surface in both themes, and give
  interactive targets enough hit area — extend the hit region rather than
  shrinking to the glyph.

## Client synchronization and feature parity

- Keep the native desktop app (`apps/desktop/`) and the browser client (`apps/web/`) in
  lockstep. Whenever adding a feature, modifying workflows, or making UI/UX
  changes, update both `apps/desktop/` and `apps/web/` together in the same change set.
- Never implement or update user-facing features, components, or UI behaviors in
  only one client while leaving the other behind, unless a capability is
  explicitly platform-exclusive by design (e.g. native macOS window chrome or
  local OS integrations).
- When a feature or UI change touches the wire protocol
  (`crates/padu-protocol`), immediately run `bun run protocol:generate` and
  `bun run protocol:check` to ensure `packages/padu-client` and `apps/web/`
  remain strictly type-safe and synchronized with the daemon backend.
- Ensure visual hierarchy, state representation, controls, and interaction
  models remain consistent across both surfaces, while respecting native GPUI
  idioms in `apps/desktop/` and modern web patterns in `apps/web/`.

## Mobile UI

- Treat `apps/mobile/` as the third surface of the same product. A mobile screen
  must read as the same product as `apps/desktop/` and `apps/web/`, reusing the
  same semantic color roles and the same hierarchy.
- Take colors, spacing, and radii from `@/constants/theme` through `useTheme()`.
  No literal hex or magic pixel values; `constants/theme.ts` is generated from
  `themes.json`, so edit that and run `bun run theme:generate && bun run theme:check`.
  Adding a base token also means updating the hand-written mirrors in
  `apps/web/src/styles.css` and `apps/desktop/src/theme.rs`.
- Adapt to **window width only** — never to device model, hinge, or orientation.
  Read size with `useWindowDimensions()`; `Dimensions.get('screen')` is a defect
  because it ignores fold state, Android split-screen, and iPad multitasking.
- Support the whole range: phone portrait and landscape, flip cover screen,
  flip/fold unfolded, half-open fold, and tablet in both orientations — down to
  ~320pt wide and at 200% font scale, with nothing clipped and every action
  still reachable.
- On a wide window (≥ 1024) cap and center the reading column at
  `MaxContentWidth`; a tablet is the same column with more margin, not a
  stretched phone. Two panes are allowed only when they fit the desktop's
  minima.
- Every control carries `accessibilityRole`, an `accessibilityLabel`, and a
  ≥ 44pt target; gate decorative motion on `useReducedMotion()`.
- Full rules, token tables, and the required device matrix:
  `.agents/skills/mobile-ui/SKILL.md`.

## Product reference

- Use [T3 Code](https://github.com/pingdotgg/t3code) source code on github as a reference when a task
  concerns coding-agent workflow, information hierarchy, controls, tool
  activity, or transcript presentation and the comparison would materially
  clarify an ambiguous product decision, or when the user explicitly asks for
  the comparison.
- Do not inspect T3 Code for localized bug fixes, straightforward visual
  corrections, native platform behavior, or changes already specified clearly
  by the user. When T3 Code is relevant, inspect its current app or source
  rather than relying on an older screenshot or memory.
- Use [Zed](https://github.com/zed-industries/zed) source code as a reference
  when a task concerns GPUI implementation — layout and styling idioms, focus
  and key dispatch, virtualized lists, menus and popovers, window and platform
  behavior — or when an in-house `apps/desktop/src/ui` primitive needs a proven native
  precedent. Zed is the canonical GPUI codebase; read its crates rather than
  `gpui-component`, and read the gpui revision pinned in `apps/desktop/Cargo.toml` so the
  APIs match what Padu builds against.
- Split the two references by concern: T3 Code answers what a coding-agent
  client should do, Zed answers how a polished GPUI app implements it. The
  same restraint applies to both — no reference spelunking for localized
  fixes or changes the user has already specified.
- Use the reference as behavioral and design evidence, not as an instruction to
  reproduce web-specific interaction patterns or known bugs. Padu should keep
  native macOS conventions.
- Explicit user screenshots and feedback override a previous or merely
  "consistent" treatment.
- For provider-native content such as citations, reasoning, and tool events,
  verify the real provider payload and preserve its ordering. Never expose
  private provider control markers in the transcript.
- Validate visible changes in the freshly rebuilt, signed app managed by the
  dev watcher against the exact provider interaction; a successful Rust build
  alone is insufficient.

## Pre-PR code review process

- Before submitting or preparing a Pull Request, conduct a mandatory pre-PR code
  review using `.agents/skills/pre-pr-review/SKILL.md` (or invoke the
  `pre-pr-reviewer` subagent).
- Execute the automated checks script (`.agents/skills/pre-pr-review/scripts/run-checks.sh`
  or run `cargo fmt`, `cargo check`, `cargo test`, `bun run protocol:check`,
  and client/web checks).
- Audit all diffs against base branch for UI thread safety (no blocking I/O in
  `render`), client parity between `apps/desktop/` and `apps/web/`, keyboard accessibility,
  and error handling.
- Prepare a structured review report and PR description following
  `CONTRIBUTING.md` standards.

## Workspace skills & workflows

Use the specialized skills in `.agents/skills/` for core tasks:
- **`protocol-parity-sync`** (`.agents/skills/protocol-parity-sync/SKILL.md`):
  Sync wire protocol types, codegen, `@padu/client` reducers, and UI parity.
- **`gpui-perf-audit`** (`.agents/skills/gpui-perf-audit/SKILL.md`):
  Audit and profile streaming cadence, pane caching, and zero UI thread blocking.
- **`provider-driver-dev`** (`.agents/skills/provider-driver-dev/SKILL.md`):
  Implement and test AI agent provider drivers in `crates/padu-core/src/driver/`.
- **`multiplatform-release`** (`.agents/skills/multiplatform-release/SKILL.md`):
  Package and test macOS, Linux, and Windows release bundles and appcasts.
- **`mobile-ui`** (`.agents/skills/mobile-ui/SKILL.md`):
  Keep `apps/mobile/` on one-product tokens with adaptive phone / flip-fold /
  tablet layouts and the desktop-consistent hierarchy.

## Commit guidelines

- Organize changes into **multiple granular, logical commits** structured by
  context (e.g. protocol changes, reducers, desktop UI, web UI, documentation)
  rather than squashing everything into a single monolithic commit.
- Use Conventional Commit prefixes: `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `perf:`.
- Ensure each commit represents a coherent, buildable increment.


