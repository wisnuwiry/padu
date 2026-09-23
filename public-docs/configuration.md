---
title: Configuration & Settings
description: Configure Padu daemon settings, provider binary overrides, and permissions in ~/.padu/settings.json.
nav: Configuration
order: 30
category: Reference
---

# Configuration & Settings

Padu persists daemon settings in a JSON file located in your user directory:

```bash
~/.padu/settings.json
```

Settings can be edited directly in the file or managed from the Padu Desktop **Settings** panel.

## Settings Format

```json
{
  "computer_use_enabled": false,
  "computer_use_allowed_apps": [],
  "disabled_providers": [],
  "provider_binary_overrides": {
    "claude": "/opt/homebrew/bin/claude",
    "codex": "/usr/local/bin/codex"
  }
}
```

## Settings Reference

### `provider_binary_overrides`
Maps provider IDs (`claude`, `codex`, `opencode`, `pi`, `cursor`, `amp`, etc.) to custom executable paths on disk. Useful when tools are installed in non-standard locations or managed by tool version managers (such as `nvm` or `mise`).

### `disabled_providers`
Array of provider IDs to hide from the provider selection menu in the user interface.

### `computer_use_enabled`
Boolean flag enabling or disabling computer automation capabilities for providers that support tool use.

### `computer_use_allowed_apps`
List of application bundle IDs granted permission for automated UI interaction.

## Daemon CLI Options

When starting the standalone daemon binary (`padu-daemon`), the following command-line flags are supported:

- `--bind <ADDRESS>`: Network socket address to bind (e.g. `127.0.0.1:34123`).
- `--allow-non-loopback`: Permits binding to external or VPN interfaces (such as Tailscale).
- `--allow-origin <ORIGIN>`: Restricts allowed browser origins for WebSocket connections.
- `--parent-pid <PID>`: Shuts down the daemon automatically when the parent process exits.

## See Also

- [Troubleshooting](/docs/troubleshooting) — Fixing `PATH` resolution and provider discovery.
- [Security & Privacy](/docs/security) — How Padu protects local credentials and network boundaries.
