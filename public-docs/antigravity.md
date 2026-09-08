---
title: Antigravity
description: Setup and run Google Antigravity in Padu with one-click ACP installation, Google OAuth login, and reasoning effort controls.
nav: Antigravity
order: 24
category: Providers
---

# Google Antigravity

Padu provides first-class support for Google DeepMind's **Antigravity** autonomous coding agent via the Agent Client Protocol (ACP).

With Padu, you can either install the official Antigravity ACP server with a single click or connect directly to your locally installed `agy` CLI.

---

## Does Antigravity cost extra in Padu?

No. Padu does not charge any fees or add proxy markups for Antigravity. Usage is billed or metered directly through your authenticated Google account / Gemini quota.

---

## Setting Up Antigravity in Padu Desktop

Padu Desktop includes an integrated provider management panel under **Settings → Providers → Antigravity** that handles the entire setup lifecycle:

### 1. Locate Antigravity in Settings

1. Open Padu Desktop.
2. Navigate to **Settings** (gear icon or `Cmd+,` / `Ctrl+,`) and select the **Providers** tab.
3. Find the **Antigravity** section.

### 2. Download and Install the ACP Server

If you do not have the Antigravity CLI installed on your system:

- Click the **Install ACP** button.
- Padu will automatically download, verify (SHA-256), and extract the official Google Antigravity ACP distribution into `~/.padu/providers/antigravity/`.
- During download, the button displays real-time progress: `Downloading (X%)`.
- Once complete, Padu marks the provider as ready and automatically configures the binary path.

> **Alternative (Using existing CLI):** If you already installed `agy` globally via npm or homebrew, Padu automatically discovers it on your system `PATH`. You can also manually enter the path in the **Path** field.

### 3. Sign In with Google

Antigravity requires authentication with your Google account:

1. In the Antigravity settings card, click **Sign in**.
2. Padu triggers the OAuth sign-in flow (`agy auth login`), opening your default browser to authorize your Google account.
3. Once authorized in the browser, Padu detects the local token (stored securely in `~/.gemini/antigravity-cli`) and updates your status to **Signed in**.

To log out or switch accounts, click **Sign out** to remove the saved credentials.

### 4. Binary Path Overrides

If you maintain a custom build or virtualenv binary:

- Type or paste the absolute path to `agy` into the **Path** field.
- To revert back to the automatically detected or downloaded binary, click **Reset**.
- You can also configure this in `~/.padu/settings.json`:
  ```json
  {
    "provider_binary_overrides": {
      "agy": "/path/to/your/agy"
    }
  }
  ```

---

## Provider Management Actions

The Antigravity settings card provides quick access to maintenance actions:

- **Sign in / Sign out:** Authenticate or disconnect your Google credentials.
- **Reinstall:** Re-downloads and refreshes the official ACP binary if files become corrupted or an update is available.
- **Remove saved download:** Removes the downloaded binaries from `~/.padu/providers/antigravity/` to free disk space.

---

## Running Antigravity in Padu

### Model Selection & Reasoning Effort

Padu automatically groups Antigravity models into clean base models and separates the reasoning effort controls (matching the experience in Codex and Claude):

- **Base Models:** Select from `Gemini 3.8 Flash`, `Gemini 3.7 Flash`, `Gemini 3.6 Flash`, or `Gemini 3.1 Pro`.
- **Reasoning Effort:** Choose between `Low`, `Medium`, or `High` in the dedicated **Reasoning** dropdown in the composer.
- Padu transparently maps your selection back to the ACP provider identifier (for example, `gemini-3.8-flash` + `High` becomes `gemini-3.8-flash-high`) during turn execution, in-session dynamic model switching, and Git commit generation.

### Workspaces & Sessions

- **New Session:** Select **Antigravity** from the provider dropdown in the session bar or composer.
- **Live Streaming:** Streams tokens, thought blocks, and tool executions in real-time at 120 FPS.
- **Turn Approvals:** In Supervised mode, inspect and approve or reject tool execution requests interactively.

---

## Troubleshooting

### "Install failed" or Extraction Error
Ensure `unzip` is available in your system `PATH`. On macOS and Linux systems, `unzip` is installed by default.

### "Sign in failed" / Browser Didn't Open
Run `agy auth login` directly in your terminal to view verbose error output or complete OAuth authentication manually.

### Antigravity Shows "Not installed"
1. Verify `agy` is executable and reachable in your terminal:
   ```bash
   which agy
   ```
2. In Padu Desktop, click the **Refresh** button next to Providers.
3. If using an npm prefix (e.g. `~/.npm-global/bin`), ensure that directory is present in your shell `PATH` or enter the full path in the provider settings.

---

## See also

- [Supported providers](/docs/supported-providers) — Full catalog of agents supported in Padu.
- [Providers overview](/docs/providers) — Architecture of Padu's native drivers and ACP integration.
- [Configuration](/docs/configuration) — Configure daemon settings and binary overrides.
