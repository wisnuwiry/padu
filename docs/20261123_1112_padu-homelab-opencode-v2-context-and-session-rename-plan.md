# Padu Homelab Runbook and OpenCode v2, Context, and Session Rename Plan

**Document date:** 2026-11-23
**Runbook baseline:** 2026-11-19
**Status:** Part I is operational. Part II is an implementation plan; no feature
implementation is included in this document change.
**Revision note:** This document consolidates and supersedes
`20261119_0843_padu-homelab-codex-remote-runbook.md`. The homelab runbook is
kept intact below as Part I, and the approved feature plan is in Part II.

## Part I — Homelab Codex Remote Runbook

**Scope:** Manual deployment of a Padu daemon and Codex CLI on the
`homelab` machine, reached from Padu Desktop through a normal SSH local
forward.
**Runbook status:** Operational, with the known limitations recorded in
Section 13.

## 1. Purpose

This document records the working setup that allows Padu Desktop on the
laptop to use Codex CLI on `homelab`.

It is an operational runbook, not a replacement for the SSH remote design
plan in
[`20261116_1322_skills-mentions-and-ssh-remote.md`](20261116_1322_skills-mentions-and-ssh-remote.md).
The design plan describes the intended product behavior. This runbook describes
the current manual deployment and the behavior actually observed in the
installed build.

Do not include daemon tokens, provider bearer tokens, SSH private keys, or
credential values in this document or its diagnostics.

## 2. Topology

```text
Padu Desktop on tricahyana-laptop
  |
  | ws://127.0.0.1:17650
  |
  v
OpenSSH local forward using the `homelab` config alias
  |
  | encrypted SSH connection
  |
  v
homelab: 127.0.0.1:17650
  |
  | padu-daemon.service (systemd --user)
  |
  +-- Codex CLI
  +-- Git repositories and workspace files
  +-- provider credentials and Codex configuration
```

The SSH tunnel is required only on the client side. The remote daemon is an
independent systemd user service and remains alive when the SSH session is
closed, provided linger is enabled.

## 3. Port Allocation

| Process | Host | Bind address | Purpose |
|---|---|---|---|
| Local Padu child daemon | laptop | `127.0.0.1:<ephemeral>` | Projects and agents running on the laptop |
| Remote Padu daemon | `homelab` | `127.0.0.1:17650` | Projects and agents running on `homelab` |
| SSH local forward | laptop | `127.0.0.1:17650` | Forwards Padu Desktop traffic to the remote daemon |

At the time of this audit, the local Padu child daemon was listening on
`127.0.0.1:40699`. That value is chosen at runtime with `--bind
127.0.0.1:0`; it must not be treated as stable.

The local and remote daemons must therefore use different local ports.
`17650` is reserved for the remote host and is outside the usual Linux
ephemeral port range. The local Padu process continues to use an ephemeral
port.

Only one process can own `127.0.0.1:17650` on the laptop. If a persistent
tunnel service is running, an interactive `ssh homelab` session that also
requests the same forward will report:

```text
bind [127.0.0.1]:17650: Address already in use
```

This deployment intentionally has no persistent tunnel service on the laptop.

## 4. Prerequisites

- Padu Desktop is installed on the laptop.
- The laptop can authenticate to `homelab` with key-based SSH.
- `homelab` has a working systemd user manager.
- Linger is enabled for the `homelab` user.
- Codex CLI is installed on `homelab`.
- Codex has a working provider configuration on `homelab`.
- The laptop and remote daemon use compatible Padu protocol versions.

## 5. SSH Configuration

The laptop uses the existing `homelab` alias. The relevant forward is declared
in `~/.ssh/config` under the shared `homelab` and `homelab.local` block:

```sshconfig
Host homelab homelab.local
    LocalForward 127.0.0.1:17650 127.0.0.1:17650
```

The `homelab` host also sets:

```sshconfig
LogLevel ERROR
```

This suppresses repeated messages caused by unrelated forwarded ports whose
remote services are not running. Padu-specific failures remain visible in the
Padu UI.

Start the tunnel with:

```sh
ssh homelab
```

Keep that session open while Padu uses the remote host. Closing the session
removes the laptop-side listener for port `17650`, although the remote daemon
continues running.

To verify the effective forward from the laptop:

```sh
ssh -G homelab | awk '$1 == "localforward" && $2 ~ /17650/ { print }'
ss -ltn "sport = :17650"
```

Expected result: one `localforward` entry and a listener on
`127.0.0.1:17650`.

## 6. Remote Daemon Installation

The remote daemon is installed at:

```text
~/.local/bin/padu-daemon
```

The token environment file is:

```text
~/.config/padu/daemon.env
```

The file must contain one variable:

```text
PADU_DAEMON_TOKEN=<secret>
```

Permissions must be restricted to the owning user:

```sh
chmod 600 ~/.config/padu/daemon.env
```

Generate a token once, without printing it into documentation or logs:

```sh
umask 077
token="$(openssl rand -hex 32)"
printf 'PADU_DAEMON_TOKEN=%s\n' "$token" |
  install -m 600 /dev/stdin "$HOME/.config/padu/daemon.env"
unset token
```

The systemd user unit is:

```ini
[Unit]
Description=Padu daemon
After=network.target

[Service]
Type=simple
EnvironmentFile=%h/.config/padu/daemon.env
ExecStart=%h/.local/bin/padu-daemon --bind 127.0.0.1:17650 --allow-origin https://app.padu.dev
Restart=on-failure
RestartSec=1

[Install]
WantedBy=default.target
```

Enable and start it:

```sh
systemctl --user daemon-reload
systemctl --user enable --now padu-daemon.service
```

Enable linger so the service survives logout:

```sh
loginctl enable-linger "$USER"
```

Verify:

```sh
systemctl --user status padu-daemon.service
loginctl show-user "$USER" -p Linger --value
ss -ltn "sport = :17650"
```

## 7. Codex Discovery

Codex was installed at:

```text
/home/tricahyana/.local/bin/codex
```

At the time of this audit, it resolved to Codex CLI `0.169.0`.

The systemd user-service `PATH` does not include `~/.local/bin`. Padu therefore
cannot discover Codex by command name alone. Add an absolute override to:

```text
~/.padu/settings.json
```

The relevant value is:

```json
{
  "provider_binary_overrides": {
    "codex": "/home/tricahyana/.local/bin/codex"
  }
}
```

Preserve all other settings in the file. Restart the daemon after changing it:

```sh
systemctl --user restart padu-daemon.service
```

Verify the executable and provider configuration without reading credentials:

```sh
"$HOME/.local/bin/codex" --version
awk -F' = ' '/^model =|^model_provider =|^forced_login_method =/ { print $1 " = " $2 }' \
  "$HOME/.codex/config.toml"
jq '.provider_binary_overrides' "$HOME/.padu/settings.json"
```

`codex login status` reporting `Not logged in` is not conclusive for this
deployment because Codex is configured to use a custom provider. Verify the
actual provider path with a minimal execution instead:

```sh
"$HOME/.local/bin/codex" exec \
  --skip-git-repo-check \
  --sandbox read-only \
  "Reply with exactly READY and nothing else."
```

This test consumes provider usage. Run it only when a live provider check is
necessary.

## 8. Padu Desktop Host Configuration

The remote host entry must use a direct WebSocket address:

```text
Name: Homelab
Transport: Default/Direct
Address: ws://127.0.0.1:17650
```

Use `ws://`, not `wss://`. The SSH tunnel provides transport encryption but
does not make the daemon a TLS endpoint.

The host token must match `PADU_DAEMON_TOKEN` on `homelab`.

### 8.1 Token behavior in the tested build

The tested desktop binary identifies itself through an archive named `0.1.9`,
while the remote daemon reports an embedded product version of `0.1.2`. Both
sides report protocol version `10`.

The tested desktop binary does not contain the `secret-tool` integration used
by the newer source tree. A token stored only in the laptop Secret Service is
therefore not seen by this binary. The host UI continues to show `No Token`,
and connection attempts fail with:

```text
daemon rejected connection: authentication failed
```

For this build, enter the same daemon token in the **Edit Host** token field.
The token is a full-control capability and must be treated as a secret.

The remote source of truth is:

```text
homelab:~/.config/padu/daemon.env
```

Do not print the token in terminal output captured for diagnostics.

## 9. Remote Project Limitation

The tested desktop build displays this error when **Open project folder** is
used while the active host is remote:

```text
Adding a daemon-host project needs a remote path picker, which is not available yet
```

The daemon supports remote paths, but this desktop build lacks the remote
directory picker required by the button. This is separate from Codex and daemon
connectivity.

### 9.1 Current project

The following project was added directly through the daemon RPC and is
available to Padu:

```text
Name: Padu (homelab)
Path: /home/tricahyana/repositories/padu
```

Use this project to start a remote Codex task. Do not use the local folder
picker for a remote host.

### 9.2 Adding another project

Until the desktop remote picker lands, add another project through the
authenticated daemon WebSocket protocol:

1. Connect to `ws://127.0.0.1:17650` through the SSH tunnel.
2. Send `hello` with protocol version `10` and the daemon token.
3. Send `loadTaskState` and preserve the returned projects and sessions.
4. Append a `Project` with an absolute remote path such as:

   ```text
   /home/tricahyana/repositories/<repository>
   ```

5. Send `saveTaskState` with the complete returned project list and the
   unchanged session list.
6. Confirm the new path with a subsequent `loadTaskState`.

`saveTaskState` is merge-oriented for projects, but preserving the loaded
session list avoids accidentally replacing live task metadata.

## 10. Verification Checklist

Run these checks from the laptop unless noted otherwise.

| Check | Command | Expected |
|---|---|---|
| SSH forward is configured | `ssh -G homelab` | Includes `localforward [127.0.0.1]:17650 [127.0.0.1]:17650` |
| SSH session owns the local port | `ss -ltnp "sport = :17650"` | `ssh` is listening on `127.0.0.1:17650` |
| Remote service is active | `ssh homelab 'systemctl --user is-active padu-daemon'` | `active` |
| Remote listener exists | `ssh homelab 'ss -ltn "sport = :17650"'` | Listener on `127.0.0.1:17650` |
| Codex is discoverable | `ssh homelab 'jq ".provider_binary_overrides" ~/.padu/settings.json'` | Contains the absolute Codex path |
| Codex runs | `ssh homelab '~/.local/bin/codex --version'` | Prints a version |
| Padu connects | Padu host **Connect** | Host becomes connected |

An authenticated WebSocket probe should receive a server `hello` with
`protocolVersion: 10`. Do not include the token in probe output.

The daemon-side provider probe used during this deployment returned:

```text
codex_installed=True
codex_path=/home/tricahyana/.local/bin/codex
codex_version=0.169.0
model_count=6
```

## 11. Troubleshooting

| Symptom | Meaning | Action |
|---|---|---|
| `bind [127.0.0.1]:17650: Address already in use` | Another local process already owns the forward | Close the duplicate tunnel or remove the duplicate `LocalForward` |
| `could not connect to Padu daemon` | No listener exists on the laptop | Start `ssh homelab` and verify `17650` locally |
| `daemon rejected connection: authentication failed` | The desktop sent a missing or stale token | Enter the current `PADU_DAEMON_TOKEN` in the host Edit dialog |
| UI shows `No Token` | This binary did not consume the OS keyring entry | Use the host Edit dialog for this build |
| `remote path picker ... not available yet` | The desktop lacks the remote folder picker | Add the project through daemon RPC or use an existing remote project |
| Repeated `channel ... open failed: Connection refused` | Another forwarded local port points at a remote service that is down | Not necessarily Padu; identify the port and start that service or stop forwarding it |
| Padu disconnects when the SSH terminal closes | The local forward disappeared | Keep `ssh homelab` open; the remote daemon itself can remain alive |
| Codex is not listed in Padu | The daemon cannot discover the Codex binary | Set `provider_binary_overrides.codex` and restart the daemon |
| Codex reports `Not logged in` but a custom provider is configured | OpenAI login status does not describe custom-provider access | Run the minimal `codex exec` provider test |
| SSH log is noisy while unrelated services poll dead forwards | OpenSSH reports channel failures for any forwarded port | Set `LogLevel ERROR` for the alias or remove unused forwards |

## 12. Security and Secret Handling

- The Padu daemon token is a full-control capability for the remote daemon.
- Store `daemon.env` with mode `0600`.
- Never place the token in a URL, process argument, issue, screenshot, or
  committed document.
- The daemon binds to remote loopback only. Do not expose `17650` directly to
  the LAN or internet.
- SSH is the transport security boundary for this manual setup.
- Rotate the daemon token if it is exposed. Update `daemon.env`, restart the
  service, and update the token in the Padu host entry.
- Rotate the Codex provider credential if it was exposed. Update its Codex
  configuration separately; it is not the Padu daemon token.
- Redact secrets from logs and terminal transcripts before sharing them.

## 13. Known Limitations

1. The tested desktop build needs the token pasted into the host form; it does
   not consume the Secret Service entry created for the newer source path.
2. The tested desktop build cannot open a remote folder picker. Remote projects
   must already exist or be added through daemon RPC.
3. The SSH local forward is tied to the interactive SSH session. This
   deployment intentionally does not use a local systemd tunnel service.
4. The archive name says `0.1.9`, but the extracted daemon reports embedded
   version `0.1.2`. Protocol `10` matches, but release metadata should be
   rebuilt before treating the artifact as a true `0.1.9` release.

## 14. Cleanup and Rollback

Stop and remove the remote daemon service:

```sh
ssh homelab '
  systemctl --user disable --now padu-daemon.service
  rm -f ~/.config/systemd/user/padu-daemon.service \
        ~/.config/padu/daemon.env \
        ~/.local/bin/padu-daemon \
        ~/.local/bin/padu-daemon.previous
  systemctl --user daemon-reload
'
```

Do not run that block if the user still needs the remote projects or Codex
sessions.

The laptop application backup created during this deployment is stored under a
timestamped path matching:

```text
~/.local/padu.app.backup-*
```

Keep it until the new application build has been exercised successfully.

## 15. Related Documentation

- [Linux installation and SSH remote hosts](linux.md)
- [Skill mentions and SSH remote implementation plan](20261116_1322_skills-mentions-and-ssh-remote.md)
- [Provider integrations](providers.md)
- [Streaming render performance](performance.md)

---

## Part II — OpenCode v2, Session Rename, and Claude Context Plan

### 16. Scope and Status

This part defines the implementation plan for three changes:

1. Add first-class support for OpenCode v2 while preserving the existing
   OpenCode v1 provider.
2. Propagate user-driven Padu session renames to supported harnesses.
3. Correct Claude Code context-window accounting for non-Claude and custom
   models whose effective context budget is below 1M tokens.

The plan is based on the repository state and harness versions observed on
2026-11-23:

| Item | Observed state |
| --- | --- |
| OpenCode binary | `v2.3.2` |
| OpenCode v2 API info | API version `2.1.3` |
| OpenCode v2 event stream | SSE at `/api/event` |
| OpenCode model catalog | `/api/model` |
| Claude Code | `2.1.330` |
| Claude compact-window environment | `CLAUDE_CODE_AUTO_COMPACT_WINDOW=400000` |

No implementation is part of this plan branch. Runtime behavior, protocol
changes, provider data, and harness configuration remain unchanged until a
separate implementation batch is approved.

### 17. Product Decisions

#### 17.1 OpenCode

OpenCode v1 and OpenCode v2 will be separate public providers. This avoids a
single driver that must support two incompatible API and event contracts.

| Decision | Final behavior |
| --- | --- |
| Provider model | Two public providers: `OpenCode v1` and `OpenCode v2` |
| Existing provider identity | `ProviderKind::OpenCode` remains v1 with serialized ID `openCode` |
| New provider identity | Add `ProviderKind::OpenCodeV2` with serialized ID `openCodeV2` |
| Command | Both providers use the `opencode` command, with independent binary overrides |
| Native session cursor | v1 keeps `ProviderResumeCursor::OpenCode`; v2 uses a separate cursor variant |
| Driver implementation | Separate v1 and v2 drivers; no shared compatibility parser |
| Unsupported binary | The incompatible provider remains visible in Settings as unavailable with a reason |
| Existing v1 sessions | Stay v1; there is no automatic or manual upgrade to v2 |
| Missing v1 native data | Mark the session as not resumable and ask the user to provide the v1 data path |
| Rename support | v1 stores the title only in Padu; v2 uses the native v2 rename API |

#### 17.2 OpenCode v2 storage and settings isolation

OpenCode v2 owns the standard OpenCode directories currently present on the
machine. OpenCode v1 receives isolated directories so the two providers do not
share models, credentials, sessions, state, skills, MCP configuration, or cache.

| Data | OpenCode v2 | OpenCode v1 |
| --- | --- | --- |
| Config | `~/.config/opencode` | Dedicated Padu-owned v1 XDG root |
| Auth and sessions | `~/.local/share/opencode` | Dedicated Padu-owned v1 XDG root |
| State and service files | `~/.local/state/opencode` | Dedicated Padu-owned v1 XDG root |
| Cache | `~/.cache/opencode` | Dedicated Padu-owned v1 XDG root |

Padu sets provider-specific `XDG_CONFIG_HOME`, `XDG_DATA_HOME`,
`XDG_STATE_HOME`, and `XDG_CACHE_HOME` values for the child process. OpenCode
continues to append its own `opencode` directory according to its normal
convention.

Padu must not write persistent OpenCode configuration. Settings and
credentials are managed outside Padu. Provider Settings will show the resolved
config, data, state, and cache paths as read-only, copyable text.

Padu may inject ephemeral process environment for Padu-owned integration
features such as MCP or Computer Use. Those values must not be persisted into
the user's OpenCode configuration.

#### 17.3 Session rename

Manual titles remain authoritative in Padu. Provider-generated titles continue
to populate `auto_title` and must never overwrite a manual title.

| Provider | Native rename behavior |
| --- | --- |
| Codex | Use Codex's native thread name operation |
| Claude Code | Use Claude's native session rename behavior when a live runtime exists |
| OpenCode v2 | `PATCH /api/session/{sessionID}` with the new title |
| OpenCode v1 | Padu-only title; no native rename |
| All other providers | Padu-only title |

Rename behavior is intentionally one-shot:

1. The user commits a rename in Padu.
2. Padu updates and persists the manual title immediately.
3. If a compatible runtime is active, Padu performs one native rename attempt.
4. If the native operation fails, Padu shows one warning and stops.
5. Padu does not retry while the runtime stays active.
6. Padu does not retry when the session or application is opened again.

No pending-rename queue is persisted. Reopening Padu must not send a rename
request that the user did not initiate in that session.

#### 17.4 Claude context budget

Padu must not use `modelUsage[*].contextWindow` as the primary budget for a
Claude session with a custom model. It also must not fall back to the largest
window in the result map, because that may select a title-generation model or a
different provider entry.

The effective budget is resolved in this order:

1. Read `CLAUDE_CODE_AUTO_COMPACT_WINDOW` from the same environment used to
   launch the Claude child process.
2. Parse a plain integer or a value with a `k` or `M` suffix.
3. If the environment value is absent or invalid, inspect the effective model
   identifier.
4. A model identifier containing `[1m]` resolves to `1,000,000` tokens.
5. A model identifier without `[1m]` resolves to `250,000` tokens.
6. If no model identifier can be determined, the budget is `Unknown`.

The current machine observes `CLAUDE_CODE_AUTO_COMPACT_WINDOW=400000`, so the
effective Claude budget is 400K tokens.

Padu does not set or overwrite `CLAUDE_CODE_AUTO_COMPACT_WINDOW`,
`CLAUDE_CODE_MAX_CONTEXT_TOKENS`, `CLAUDE_CODE_DISABLE_1M_CONTEXT`, or any
other harness setting. Users configure those values outside Padu.

The Padu-side Claude context-window selector `200k / 1m` is removed. Existing
stored `context_window` values on Claude sessions are ignored so old Padu
settings cannot silently alter a harness-defined effective budget.

The context meter will be labeled `Effective context` across providers. Its
tooltip reports the source of the denominator:

| Source | Tooltip meaning |
| --- | --- |
| Harness environment | `CLAUDE_CODE_AUTO_COMPACT_WINDOW` |
| Claude model suffix | `Model name ([1m])` |
| Claude non-1M fallback | `Default for non-[1m] model` |
| Provider stream or catalog | `Provider-reported context window` |
| No usable model or value | `Context budget could not be determined` |

When the token count exceeds the effective budget, the visual gauge remains
full at 100 percent. The raw token count and budget remain visible, and the
text must communicate that the budget is exceeded instead of hiding the
overflow.

### 18. Protocol and Persistent Model Changes

The implementation will update the wire protocol and regenerate TypeScript
bindings with `bun run protocol:generate` and `bun run protocol:check`.

#### 18.1 Provider identity and cursors

Add:

- `ProviderKind::OpenCodeV2`
- `ProviderResumeCursor::OpenCodeV2 { session_id }`

Preserve:

- `ProviderKind::OpenCode` serialized as `openCode`
- `ProviderResumeCursor::OpenCode { session_id }`

The legacy identity must remain stable so existing sessions, profiles, pins,
archives, and provider settings are not silently migrated.

#### 18.2 Context usage source

Extend context usage with a serialized source discriminator. The exact name may
be `ContextWindowSource`, and the generated TypeScript must expose the same
values in camelCase.

Suggested variants:

- `HarnessEnvironment`
- `ClaudeModelOneMillion`
- `ClaudeModelDefault`
- `Provider`
- `Unknown`

The source field must survive event reduction and session hydration so the
meter can explain why a denominator exists or why it is unknown.

#### 18.3 Rename command and warning

Add a daemon-owned command such as:

```rust
RenameSession {
    session_id: Uuid,
    title: String,
}
```

The command validates and trims the title, persists the manual title, and
returns the updated session metadata. If native synchronization fails, the
daemon returns a non-fatal warning result or emits a dedicated warning event;
the Padu title must remain saved.

The wire contract must not create a persistent retry token. One client action
means at most one native attempt.

### 19. Daemon and Driver Design

#### 19.1 Provider registry

Add one agent profile for OpenCode v2. Both OpenCode profiles receive the same
curated role tags and cost tier, but their runtime, cursor, model catalog, and
settings are independent.

Provider probing must validate the binary's major version:

| Binary | OpenCode v1 | OpenCode v2 |
| --- | --- | --- |
| v1.x | Available | Unavailable with reason |
| v2.x | Unavailable with reason | Available |
| Future binary supporting both | Available | Available |

The runtime performs a second guard against `/api/info` before accepting a v2
session. Binary discovery alone is not sufficient evidence that the expected
API is reachable.

#### 19.2 OpenCode v1 driver

The current `driver/opencode.rs` implementation remains the v1 driver. It
continues to use the existing v1 routes and event parser. It must receive the
new isolated v1 XDG roots and the v1 cursor.

OpenCode v1 rename remains Padu-only.

#### 19.3 OpenCode v2 driver

Create a separate v2 driver with direct routes for:

| Operation | v2 route |
| --- | --- |
| Health and capability detection | `GET /api/info` |
| Model catalog | `GET /api/model` |
| Create session | `POST /api/session` |
| Read session | `GET /api/session/{sessionID}` |
| Rename | `PATCH /api/session/{sessionID}` |
| Prompt | `POST /api/session/{sessionID}/prompt` |
| Steering | Prompt with `delivery: "steer"` |
| Interrupt | `POST /api/session/{sessionID}/interrupt` |
| Messages | `GET /api/session/{sessionID}/message` |
| Session context | `GET /api/session/{sessionID}/context` |
| Permission list | `GET /api/session/{sessionID}/permission` |
| Permission reply | `POST /api/session/{sessionID}/permission/{requestID}/reply` |
| Form list | `GET /api/session/{sessionID}/form` |
| Form reply | `POST /api/session/{sessionID}/form/{formID}/reply` |
| Event stream | `GET /api/event` |

The v2 event stream is an SSE contract and remains volatile by definition. A
disconnect can miss events, so the driver must reconnect and refresh session
messages/context before continuing live event reduction.

The v2 event translator must cover at least:

- `server.connected`
- `session.text.started`, `session.text.delta`, `session.text.ended`
- `session.reasoning.started`, `session.reasoning.delta`,
  `session.reasoning.ended`
- `session.tool.input.started`, `session.tool.input.delta`,
  `session.tool.input.ended`
- `session.tool.called`, `session.tool.progress`, `session.tool.success`,
  `session.tool.failed`
- `session.step.started`, `session.step.ended`, `session.step.failed`
- `session.execution.started`, `session.execution.succeeded`,
  `session.execution.failed`, `session.execution.interrupted`
- `session.renamed`
- `permission.asked`, `permission.replied`
- `form.created`, `form.replied`, `form.cancelled`
- `session.error`

Unknown event types are ignored, retained in debug-level diagnostics, and must
not terminate the session.

#### 19.4 Claude driver

The Claude driver will:

1. Resolve the effective context budget from the child-process environment and
   model identifier.
2. Emit `Effective context` source metadata with token and window updates.
3. Stop using the largest `modelUsage.contextWindow` fallback.
4. Ignore the legacy Padu Claude context-window selector.
5. Leave all harness environment and model configuration untouched.

The native rename operation must use one attempt and must not be replayed on
startup or resume.

#### 19.5 Codex and other rename-capable harnesses

Codex keeps its existing native thread-name operation. The daemon must route
the manual title through the live driver once rather than generating an
automatic title.

Harnesses without a proven native rename operation receive a Padu-only title
and no warning beyond the generic capability behavior.

### 20. Client and UI Parity

All three clients must present the same provider and session semantics.

#### 20.1 Desktop

- Add the OpenCode v2 provider icon, label, availability reason, and settings
  row.
- Show read-only OpenCode v1/v2 config, data, state, and cache paths.
- Remove the Claude `200K / 1M` selector.
- Label the usage lane `Effective context` and display the source in the
  tooltip.
- Clamp the visual gauge to 100 percent while preserving raw token values.
- Send rename changes through the new daemon command.
- Show one non-blocking warning when native rename fails.
- Keep keyboard focus, focus-visible treatment, and reduce-motion behavior
  correct for new controls.

#### 20.2 Web

Mirror desktop behavior for:

- OpenCode v1/v2 provider availability and settings.
- Read-only configuration paths.
- Claude context selector removal.
- Effective-context label and source tooltip.
- Rename command, one-shot native attempt, and warning.
- Gauge clamping and unknown-budget state.

#### 20.3 Mobile

Mirror the same provider capability and context semantics using adaptive
layouts at the required phone, fold, and tablet widths. Every new control must
have an accessibility role, label, and at least a 44pt target.

### 21. Migration and Compatibility

#### 21.1 Provider identity

No automatic session migration occurs. Existing `openCode` rows remain v1.
Users who want OpenCode v2 start new sessions under `openCodeV2`.

#### 21.2 Native OpenCode data

The standard OpenCode directories are assigned to v2 because the observed
database uses the v2 session schema. v1 receives isolated directories.

If a v1 session exists without a usable v1 native store, Padu:

1. Keeps the Padu transcript and metadata.
2. Marks the session as not resumable at the native harness layer.
3. Shows the expected v1 config/data/state/cache paths.
4. Asks the user to supply or restore the v1 data path.

Padu must not copy, move, or rewrite the standard v2 directories while
handling a v1 session.

#### 21.3 Claude context

No harness setting migration is performed. Padu reads the effective setting
that already exists outside Padu. Stored Padu `context_window` values for
Claude are ignored but may be cleared during a safe later migration.

### 22. Implementation Sequence

#### Phase 1 — Protocol and persistent model

- Add OpenCode v2 provider and cursor identity.
- Add context-window source metadata.
- Add the rename command and non-fatal warning contract.
- Regenerate and verify TypeScript bindings.
- Add protocol tests for serialization, deserialization, and generated type
  parity.

#### Phase 2 — Daemon and drivers

- Add OpenCode v2 provider registry/profile metadata.
- Add major-version probing and unavailable reasons.
- Configure isolated v1 and standard v2 environment roots.
- Keep the v1 driver separate.
- Implement the v2 driver and event translator.
- Implement Claude effective-budget resolution.
- Implement one-shot rename routing and native capability checks.

#### Phase 3 — Clients

- Add provider availability and path displays.
- Remove the Claude window selector.
- Add effective-context source presentation.
- Add rename command integration and one-shot warning behavior.
- Keep desktop, web, and mobile behavior synchronized.

#### Phase 4 — Verification

The implementation batch must run the project checks required by the changed
surfaces:

| Area | Required verification |
| --- | --- |
| Protocol | `bun run protocol:generate` and `bun run protocol:check` |
| Rust | `cargo fmt`, `cargo check`, and affected tests |
| Client | Typecheck and tests for `@padu/client` |
| Web | Typecheck and tests |
| Mobile | Typecheck and tests |
| Desktop | Affected Rust tests and interactive smoke validation |

### 23. Acceptance Criteria

#### OpenCode

- OpenCode v1 and v2 appear as separate providers.
- A v1 binary makes v1 available and marks v2 unavailable with a reason.
- A v2 binary makes v2 available and marks v1 unavailable with a reason.
- v2 uses the `/api/*` routes listed in Section 19.3.
- v1 and v2 do not share config, auth, sessions, state, cache, models, skills,
  or MCP settings.
- Padu does not persist configuration for either provider.
- Paths are visible as read-only values.
- Existing v1 sessions remain v1 and are not upgraded automatically.
- A missing v1 native store produces an explicit not-resumable state.

#### Rename

- Rename in Padu always updates the manual title.
- Manual titles keep precedence over provider-generated titles.
- Codex, Claude, and OpenCode v2 receive at most one native rename attempt.
- OpenCode v1 and unsupported harnesses remain Padu-only.
- A failed native rename keeps the Padu title and shows one warning.
- Reopening Padu does not retry the rename.

#### Context

- Claude uses `CLAUDE_CODE_AUTO_COMPACT_WINDOW` when valid.
- A `[1m]` model resolves to 1,000,000 tokens when the environment value is
  absent or invalid.
- A non-`[1m]` model resolves to 250,000 tokens when the environment value is
  absent or invalid.
- A missing model produces `Unknown`.
- Padu no longer falls back to the largest `modelUsage.contextWindow`.
- Padu does not modify Claude or other harness settings.
- The UI labels the meter `Effective context` and exposes the source.
- Visual percentage is clamped to 100 percent while raw values remain visible.

### 24. Non-Goals

- No implementation in this plan branch.
- No automatic OpenCode v1 to OpenCode v2 session upgrade.
- No direct editing of unsupported harness session files.
- No persistent provider configuration writes from Padu.
- No background rename retries.
- No Padu-owned Claude context or compaction settings.
- No changes to delegated context compaction in this plan; its implementation
  and review are tracked separately on `feat/compaction`.

### 25. Related Work

- [Transcript compaction implementation plan](20261120_2041_transcript-compaction-plan.md)
- [Provider integrations](providers.md)
- [Streaming render performance](performance.md)
- [Linux installation and SSH remote hosts](linux.md)
