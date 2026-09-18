# Changelog

All notable changes to Padu. This file is the **source of truth for the release
notes shown in the in-app updater**: [`scripts/release.ts`](scripts/release.ts)
extracts the section whose heading matches the version being released
(`MARKETING_VERSION`) and publishes it next to the update, so Sparkle shows it in
the update prompt.

Format follows [Keep a Changelog](https://keepachangelog.com). Add a new
`## [<version>]` section at the top for each release, matching the version you
set in the Xcode project.

Write release notes for the final product users receive, not the development
history. When a feature is still unreleased, fold its fixes and refinements into
the original feature bullet instead of adding separate entries for them.

## [unreleased]

## [0.1.7] - 2026-09-18

### Added

- **Sidebar Provider Toggle**: Added appearance setting to show/hide provider icons in the sidebar, synced across desktop, web, and persistence layer.
- **Conversation Backgrounds in Empty View**: Show conversation background in empty and onboarding views on desktop and web.
- **Provider Account Line**: Show signed-in user and subscription plan in expanded provider settings, with background plan-usage refresh.
- **Cleaner Session Titles**: Normalize auto titles — strip markdown, humanize code paths/casing, drop provider placeholders.
- **Antigravity Auth Flow**: Capture auth URL with copy-link control and modularized settings UI.

### Improved

- **Composer Responsiveness**: Wrapping toolbar clusters, truncated trigger chips, and clamped overlays to prevent overflow.
- **Antigravity Identity**: Read identity from Antigravity's own OAuth token files on all platforms instead of macOS keychain.

### Fixed

- **Title Propagation**: Route driver, catalog, and resume titles through shared normalizer in core, client reducer, and web.

## [0.1.6] - 2026-09-18

### Added

- **What's New Release Notes**: Added an in-app "What's new" dialog that appears once after each update, with a sidebar button, an About-page entry, and notes for the current version bundled at build time.
- **Conversation Backgrounds**: Added per-conversation background customization with always-cover rendering in appearance settings.
- **Activity Previews**: Added a four-row activity preview with per-file-type icons and file-target helpers for tool activity groups.
- **Richer Activity Disclosures**: Added syntax-highlighted code blocks and compare-style diffs for edit activities in the transcript.
- **Antigravity Plan Mode**: Added Antigravity plan-mode support with multiple-choice questions, an activity stream, inlined plan artifacts, and `ask_question` recognition in transcripts.
- **Provider Icons in Sessions**: Added provider icons beside session titles in the sidebar.
- **Out-of-Workspace File Links**: Open file links outside the workspace with the system default app on desktop.
- **Cleaner Agent Output**: Cleaned OpenCode read envelopes, pagination notices, and file-read arguments from transcripts, and present file creations as creates instead of edits.

### Improved

- **Command Code Driver**: Improved Command Code activities, question handling, and process teardown.
- **Performance**: Cached activity-disclosure highlighting to keep transcript scrolling smooth.

### Fixed

- **Shortcuts**: Required the primary modifier for the notes-collapse shortcut.
- **Plan Detection**: Fixed multiplatform plan-file detection and path extraction, and hardened Antigravity plan inlining and scope auto-approve.
- **Transcript Robustness**: Fixed file-read envelope stripping across drivers and clients.

## 0.1.5 - 2026-09-14

### Added

- **Qoder and Command Code Providers**: Added Qoder ACP support and Command Code as a first-class coding-agent provider across the desktop workflow, including provider setup, model/session handling, and client parity.
- **Rich Markdown Rendering**: Added native Mermaid diagrams and LaTeX math rendering to Markdown previews, alongside richer code blocks and constrained raw HTML.
- **Inline File References**: Added inline file path chips and file mention support for referencing workspace files from conversations.
- **Review and Workspace Navigation**: Added collapsible review files, review shortcuts, settings breadcrumbs, and synchronized keyboard navigation improvements.

### Improved

- **Markdown and Transcript Experience**: Improved preview refresh behavior, text selection, transcript link handling on Windows, and file-type icon presentation.
- **Notes and Provider Reliability**: Improved adding notes from projectless sessions and hardened Command Code process teardown and Antigravity authentication refresh handling.

### Fixed

- **Rendering Stability**: Fixed duplicate rich-render waits, raw HTML rendering, and Markdown preview refresh issues.
- **Conversation Input**: Preserved file mentions as plain text and prevented copying an empty prompt.

## 0.1.4 - 2026-09-10

### Added

- **Notes**: Introduced a new dedicated notes workspace with project-wide search, note capture from conversations, embedded note previews, and keyboard navigation.
- **Antigravity Installation**: Added a cross-platform provider download manager with cancellable installation and direct Windows ACP launching.

### Improved

- **Notes Experience**: Refined note navigation, composer cards, markdown preview sizing, editing layout, and workspace integration.
- **Desktop Reliability**: Added updater loading feedback, improved browser WebView lifecycle management, and cached notes-pane rendering for smoother performance.

### Fixed

- **Notes State and Navigation**: Fixed note persistence, stale updates, sidebar visibility, workspace navigation, and preview scrolling.
- **Browser and Provider Lifecycle**: Fixed WebView cleanup and Antigravity provider startup behavior across platforms.

## 0.1.3 - 2026-09-08

### Added

- **Antigravity Provider Support**: Added Antigravity as a supported coding agent provider, including installation detection, model grouping, settings configuration, and provider-specific session handling.
- **File Search Command Palette**: Added a command palette action for quickly finding files in the current workspace.
- **Task Pinning and Archive Browsing**: Added task pinning and archive metadata so important tasks are easier to find and revisit.
- **Inline File Mentions**: Added file mention support with keyboard shortcuts, workspace mention staging, and attachment chips for referencing files in conversations.

### Improved

- **Review Diff Controls**: Added file-level controls and clearer navigation for reviewing changes in the diff view.
- **Workspace File Actions**: Refined file panel actions and consolidated destructive actions around shared confirmation dialogs.
- **Desktop Dialogs**: Introduced reusable dialog and button primitives for a more consistent desktop interaction experience.

### Fixed

- **Antigravity Provider State**: Fixed provider state detection and model propagation for Antigravity sessions and commit-message generation.

## 0.1.2 - 2026-09-07

- **Right Panel Full-Size Mode**: Maximize the right panel to full-size view with dedicated keyboard shortcuts (`ctrl+tab` / `ctrl+shift+tab` tab navigation), unified headers, and balanced transcript layout sizing.
- **File Tree Workspace Actions & Context Menus**: Added comprehensive file actions in the right panel working tree, including context menus for creating files/folders, danger confirmation delete dialogs, path copying, and `.gitignore` file filtering.
- **Notifications Settings & Audio Alerts**: Added a dedicated Notifications tab in Settings with system notification permission inspection, customizable alert sound preview, and bell status indicator.
- **About Settings Page & In-App Updater**: Added an About settings page displaying app build info, environment details, project links, and an in-app updater check state with progress indicator.
- **Theme Preview Cards**: Revamped appearance settings with embedded SVG theme preview cards for instant visual theme selection.
- **Daemon Origin Whitelist Management**: Added dynamic multi-origin whitelist configuration in settings for secure remote and local daemon connectivity.
- **REPL & Process Watchdog Reliability**: Enhanced `padu_js_repl` with parent process watchdog monitoring to prevent orphan processes, and ensured clean teardown during computer-use sessions.

## 0.1.1 - 2026-09-04

- **Indonesian Language Support**: Added complete Indonesian (Bahasa Indonesia) localization to the desktop client, navigation, and settings, with automatic system locale detection.
- **Apple Developer ID Signing & Notarization**: Official macOS releases are now Developer ID-signed and Apple-notarized, eliminating Gatekeeper warnings.
- **Provider Driver Diagnostics**: Added `padu-provider-test` CLI harness for validating and debugging AI agent provider drivers.
- **Native Build Performance**: Integrated compiler cache optimizations (`sccache`) across native desktop packaging and build pipelines.

## 0.1.0 - 2026-09-03

### Highlights

Padu is a fast, GPU-accelerated native control plane for local coding agents. Built in Rust with GPUI, it keeps your projects, sessions, and transcripts local on your machine with seamless multi-platform support across macOS, Linux, and Windows.

### Key Features

- **Local Agent Integrations**: First-class support for multiple coding agent CLIs, including Claude Code, Codex CLI, Cursor CLI, Amp, OpenCode, Grok Build, Pi, Kimi Code, and Fx.
- **Native GPUI Desktop Client**: Sub-millisecond input response, smooth 120Hz scrolling, native macOS/Linux/Windows window styling, and customizable dark/light theme support.
- **Standalone Daemon & Browser Client**: Run headlessly with `padu-daemon` and access your workspace remotely or locally using the companion `@padu/web` client.
- **Multi-Host Profile Support**: Seamlessly configure and switch between local and remote daemons directly from the sidebar.
- **Multi-Session Workspace & Branching**: Manage concurrent coding sessions, track session checkpoints, inspect git diff reviews, and switch branches without leaving the app.
