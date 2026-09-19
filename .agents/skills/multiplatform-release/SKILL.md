---
name: multiplatform-release
description: >-
  Build, sign, and test macOS/Linux/Windows release bundles and appcast feeds.
---

# Multi-Platform Release & Packaging Skill

Toolchains/signing per OS: [prerequisites](./references/platform-prerequisites.md).
Local test walkthrough: [testing bundles](./examples/testing-local-bundle.md).

## When to use

Version releases, packaging-script changes, signing/notarization debugging,
or appcast feed work.

## Workflow

1. **Version** — bump `Cargo.toml` + `package.json`; `bun ./scripts/changelog.ts`.
2. **Bundle** — `./scripts/bundle.sh release` (macOS) ·
   `./scripts/bundle-linux.sh` (Linux) · `bun scripts/bundle-windows.ts` (Windows).
3. **Appcast** — `bun ./scripts/appcast{,-linux,-windows}.ts`.
4. **CI** — confirm secrets wiring in `.github/workflows/release.yml`.
