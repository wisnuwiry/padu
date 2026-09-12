<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="apps/landing/public/padu.svg">
    <img alt="Padu" src="apps/landing/public/padu.svg" width="240">
  </picture>
</p>

<p align="center">
  <em>One native app for all your coding agents.</em>
</p>

<p align="center">
  <a href="https://github.com/wisnuwiry/padu/stargazers"><img src="https://img.shields.io/github/stars/wisnuwiry/padu?style=flat&label=Stars&labelColor=1e1e1e&color=ffd43b" alt="Stars"></a>&nbsp;
  <a href="https://github.com/wisnuwiry/padu/blob/main/LICENSE"><img src="https://img.shields.io/github/license/wisnuwiry/padu?style=flat&label=License&labelColor=1e1e1e&color=6bcb9e" alt="License"></a>&nbsp;
  <a href="https://github.com/wisnuwiry/padu/releases/latest"><img src="https://img.shields.io/github/v/release/wisnuwiry/padu?style=flat&label=Release&labelColor=1e1e1e&color=58a6ff" alt="Release"></a>&nbsp;
  <a href="https://rustc-hash.vercel.app/"><img src="https://img.shields.io/badge/Rust-1.83+-de4d3a?style=flat&labelColor=1e1e1e&logo=rust&logoColor=fff" alt="Rust"></a>&nbsp;
  <a href="https://x.com/wsme_dev"><img src="https://img.shields.io/badge/X-@wsme__dev-000000?style=flat&labelColor=1e1e1e&logo=x&logoColor=fff" alt="@wsme_dev"></a>
</p>

<p align="center">
  <a href="https://padu.dev/download">Download</a>&nbsp;·
  <a href="#overview">Overview</a>&nbsp;·
  <a href="#supported-agents">Agents</a>&nbsp;·
  <a href="#highlights">Highlights</a>&nbsp;·
  <a href="#architecture">Architecture</a>&nbsp;·
  <a href="#development">Development</a>&nbsp;·
  <a href="#license">License</a>
</p>

---

> [!NOTE]
> **Under Active Development**: Padu is in active development and early preview. You may encounter bugs, incomplete features, or rough edges. Bug reports and contributions are very welcome!

## Overview

Padu is a fast, native desktop app for working with local coding agents. Built
in Rust with [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui),
it keeps projects, sessions, and transcripts entirely on your machine.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="apps/landing/public/preview-dark.webp">
  <img alt="Padu app preview" src="apps/landing/public/preview-light.webp" width="100%">
</picture>

## Download

The recommended way to download and install Padu on **macOS**, **Windows**, and **Linux** is via:

👉 **[https://padu.dev/download](https://padu.dev/download)**

| Platform | Download / Install | Notes |
| :--- | :--- | :--- |
| **macOS** | [Download `.dmg`](https://padu.dev/download) | Apple Silicon & Intel, Developer ID signed and Apple-notarized |
| **Windows** | [Download `.exe` Setup](https://padu.dev/download) | x64 & ARM64 installers, per-user install |
| **Linux** | [Download `.tar.gz`](https://padu.dev/download) | x86_64 & ARM64 tarballs, Wayland & X11 support |

You can also find all release assets and checksums on [GitHub Releases](https://github.com/wisnuwiry/padu/releases/latest).

---

### Command Line Install

#### Linux

Install directly into `~/.local/padu.app` without root or package managers:

```sh
curl -fsSL https://padu.dev/install.sh | sh
```

#### Windows

Install per-user via PowerShell:

```powershell
# Open download page in browser:
Start-Process https://padu.dev/download
```

> ℹ️ The Windows installer installs to `%LOCALAPPDATA%\Programs\Padu` without requiring administrator privileges. If Microsoft Defender SmartScreen prompts, click **More info** → **Run anyway**.

## Supported agents

Padu works with:

- [Amp](https://ampcode.com/)
- [Claude Code](https://claude.ai/code)
- [Codex CLI](https://github.com/openai/codex)
- [Cursor CLI](https://cursor.com/)
- [Fx](https://fx.sh/)
- [Grok Build](https://x.ai/)
- [Kimi Code](https://kimi.ai/)
- [OpenCode](https://opencode.ai/)
- [Pi](https://github.com/badlogic/pi-mono)
- and [more...](https://padu.dev/agents)

Install and authenticate at least one supported agent CLI before starting Padu.
Padu detects available CLIs automatically and uses each provider's native
structured protocol and session continuity.

## Highlights

- **Unified workspace** — Keep projects and independent agent sessions in one
  native app.
- **Shared controls** — Switch models, reasoning effort, and access modes from
  a single interface.
- **Queue & steer** — Send follow-up messages while an agent is still working.
- **Rewind** — Git-backed task history with conversation-aware checkpoints.
- **Local-first** — Everything stays on your machine. No Padu account or remote
  service required.

## Architecture

The native desktop is an RPC client of the standalone `padu-daemon` process.
Provider sessions run in [`padu-core`](crates/padu-core/), behind the
authenticated, versioned WebSocket contract in
[`padu-protocol`](crates/padu-protocol/). Padu Desktop depends on
[`padu-client`](crates/padu-client/), not on the daemon implementation. The
daemon owns task SQLite data, uploaded attachments, provider-native session
forks, and all workspace filesystem and Git operations; paths returned by it
always refer to the daemon host. The desktop retains only presentation state
and a disposable preview cache.

The browser client lives at [`apps/web`](apps/web/) and uses the generated
browser transport in [`packages/padu-client`](packages/padu-client/). Its
checked-in types are generated directly from the Rust protocol, while its
WebSocket client implements the same handshake, request IDs, subscriptions,
sequence deduplication, and replay cursors as the Rust client. Run
`bun run protocol:generate` after changing a wire type and
`bun run protocol:check` to verify that generated files are current.

Projectless task workspaces live on the daemon host under
`~/.padu/projects/<date>/<slug>`. The daemon moves workspaces created by the
older `~/.padu/<date>/<slug>` layout on first load.

Configuration ownership is separate too: the Release desktop writes
`~/.padu/app.json`, while Debug stays isolated at `temp/app.json`. Daemon
provider and Computer Use settings live in `~/.padu/settings.json`. The
desktop's Settings → Daemon page can explicitly
expose the child daemon on a fixed port, configure exact browser origins, and
copy its stable authentication token. It remains loopback-only by default.

When connected to a daemon managed outside the desktop process, Padu never
interprets daemon paths on the client machine. The local folder picker and PTY
are therefore unavailable until the protocol gains daemon-host picker and
terminal-stream endpoints; files, diffs, Git, skills, usage, task state, and
attachments already use daemon RPC.

Release apps bundle and sign `padu-daemon`. Development keeps the daemon at
`target/debug/padu-debug-daemon`, allowing provider-only edits to rebuild and
replace the daemon without relaunching Padu Debug.

## Development

Development is supported on macOS, Linux, and Windows and requires
[Rust 1.96 or newer](https://www.rust-lang.org/tools/install) and
[Bun](https://bun.sh/). Linux supports both Wayland and X11, and Windows needs
the MSVC toolchain; install the native build prerequisites listed in
[CONTRIBUTING.md](CONTRIBUTING.md) first.

```sh
bun install
bun run dev
```

The embedded browser and experimental computer-use integration currently
remain macOS-only. Agent sessions, projects, transcripts, skills, usage,
diffs, file editing, and the terminal run natively on Linux and Windows.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow and checks.
Release maintainers should also read [RELEASING.md](RELEASING.md).

## Contributors

Thanks to everyone who helps make Padu better.

<a href="https://github.com/wisnuwiry/padu/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=wisnuwiry/padu" alt="Padu contributors" width="80" />
</a>

## Sponsorship

You can support the project development via
[GitHub Sponsors](https://github.com/sponsors/wisnuwiry).

## License

Padu is licensed under the [GNU General Public License v3.0 only](LICENSE).
See [NOTICE.md](NOTICE.md) for full license attribution and details.
