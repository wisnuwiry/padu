# Padu Desktop 0.1.5 Release Plan

**Release:** `v0.1.5`  
**Target date:** 2026-09-14  
**Scope:** Desktop release and its coordinated native distribution artifacts

## Release summary

Padu 0.1.5 focuses on richer Markdown and transcript presentation, expanded coding-agent support, and workflow polish. The release includes Qoder ACP and Command Code provider support, native Mermaid and LaTeX rendering, inline file references, review/navigation improvements, and reliability fixes for notes, providers, and Windows transcript links.

## Pre-release checklist

- [x] Bump the authoritative desktop version in `apps/desktop/Cargo.toml` to `0.1.5`.
- [x] Keep `apps/desktop/package.json` at `0.1.5` for workspace metadata parity.
- [x] Update the `padu` package entry in `Cargo.lock`.
- [x] Add the `0.1.5` section to `CHANGELOG.md`.
- [ ] Review the changelog against the final user-visible feature set.
- [ ] Run formatting and desktop checks:
  - `cargo fmt --package padu -- --check`
  - `cargo check --package padu`
  - `cargo test --package padu`
- [ ] Run the release-oriented client checks required for the coordinated desktop/web changes:
  - `bun run protocol:check`
  - `bun run --filter @padu/client check`
  - `bun run --filter @padu/client test`
  - `bun run --filter @padu/web typecheck`
  - `bun run --filter @padu/web test`
- [ ] Verify the freshly rebuilt desktop app with representative provider and Markdown flows.

## Packaging and publishing

1. Confirm the working tree contains only intended release changes.
2. Push the versioned change and create tag `v0.1.5`.
3. Let the Release workflow build the platform matrix:
   - macOS: `Padu-0.1.5.dmg`, `Padu-0.1.5.zip`, and `appcast.xml`
   - Linux x86_64/arm64: versioned `.tar.gz` bundles and architecture-specific appcasts
   - Windows x86_64/arm64: portable `.zip` bundles, Inno Setup installers, and architecture-specific appcasts
4. Verify the generated GitHub release is drafted with notes extracted from `CHANGELOG.md`.
5. Verify signatures/installer metadata and that all expected assets are present.
6. Publish the draft release. The release sync workflow then copies immutable artifacts and short-lived feed pointers to R2.
7. Confirm download URLs, architecture-specific feeds, and the macOS Sparkle update path.

## Smoke-test matrix

- **macOS:** Install the DMG, launch the app, run a provider session, render Mermaid/LaTeX Markdown, and check **Check for Updates…** from an older installation.
- **Linux:** Install each architecture bundle, launch a provider session, and verify the native updater can discover the release feed.
- **Windows:** Install each architecture-specific installer, verify provider startup and Windows transcript links, then confirm the portable bundle starts with its daemon.
- **Cross-client parity:** Verify provider configuration, Markdown rendering, inline file references, review navigation, and settings breadcrumbs remain consistent with the web client.

## Rollback and follow-up

- Do not republish a version with changed signed artifacts. If a release is defective, prepare `0.1.6` with the fix.
- If only an appcast or R2 pointer is incorrect, correct the feed/pointer and revalidate signatures before republishing metadata.
- Keep the `v0.1.5` tag, GitHub release notes, and R2 artifacts immutable for traceability.
