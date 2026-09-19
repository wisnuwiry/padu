---
name: gpui-perf-audit
description: >-
  Audit GPUI streaming performance (UI-thread I/O, commit/pulse cadence, pane
  caching). Use for CPU spikes, frame stutter, or animation/scrollbar changes.
---

# GPUI Performance & Streaming Audit Skill

Target: sustained streaming CPU ~<10% on 120 Hz displays. Rules verbatim in
[Cadence Invariants](./references/cadence-invariants.md) — this file is the
procedure, not a second copy of the rules.

## When to use

CPU spikes / stutter while streaming; touching animation loops, loaders,
scrollbars, pane observation, or `runtime.rs` / `ui/motion.rs` / `md/render.rs`.

## Checklist (in order)

1. **UI-thread I/O** — no `std::fs::*` / `Command::new` reached from
   `render()` or row builders (`git grep` both in `apps/desktop/src/`).
   Heavy work → `cx.background_executor().spawn`, store on entity, `cx.notify()`.
2. **Cadence** — commits ≤ ~8.3 Hz (120 ms interval, every delta kind routes
   the pump onto `StreamFrame`); pulse ≤ ~30 Hz via shared clock, never
   `with_animation(...).repeat()`; loaders on costly surfaces carry a stride.
3. **Panes** — root re-renders every frame; sidebar/transcript/right panel stay
   `Entity::cached` islands observing root; no fan-out during 200 ms slide.
4. **Scrollbars** — constant-opacity hold while streaming (zero repaints);
   one-shot wake on expiry, pulse clock through the 350 ms fade only.

## Measure

```bash
.agents/skills/gpui-perf-audit/scripts/measure-stream-cpu.sh 15 "Padu Debug"  # avg < ~12% debug
sample $(pgrep -f "Padu Debug" | head -1) 5                                    # leaf-function sampling
```

If notify rate is suspect, wire the temporary `AtomicU32` counters from
[counter-instrumentation.rs](./examples/counter-instrumentation.rs) (do not ship).
