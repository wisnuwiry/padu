//! ACP provider registration: no driver code needed — register in 3 places.
//! (Cline, Goose, Gemini CLI, Qwen, Cursor, Kimi, Grok, …)

 // 1. `crates/padu-protocol/src/model.rs`
/*
pub enum ProviderKind {
    // ...
    MyAgent, // + append to ALL
}

impl ProviderKind {
    pub fn id(self) -> &'static str { match self { /* ... */ Self::MyAgent => "myagent", } }
    pub fn display_name(self) -> &'static str { match self { /* ... */ Self::MyAgent => "My Agent CLI", } }
    pub fn short_name(self) -> &'static str { match self { /* ... */ Self::MyAgent => "MyAgent", } }
    pub fn command(self) -> &'static str { match self { /* ... */ Self::MyAgent => "myagent", } }

    // Opt into capabilities as supported:
    pub fn supports_conversation_rollback(self) -> bool {
        matches!(self, Self::Amp | Self::Claude | Self::Codex | Self::Cursor | Self::MyAgent)
    }
    pub fn supports_conversation_fork(self) -> bool {
        matches!(self, Self::Amp | Self::Claude | Self::Codex | Self::Cursor | Self::MyAgent)
    }
    pub fn supports_model_discovery(self) -> bool {
        matches!(self, Self::Claude | Self::Codex | Self::Cursor | Self::MyAgent)
    }
}
*/

 // 2. `crates/padu-core/src/driver/acp.rs` + `mod.rs`
/*
fn launch_for(provider: ProviderKind, reasoning_effort: Option<&str>) -> anyhow::Result<AcpLaunch> {
    match provider {
        // ...
        ProviderKind::MyAgent => {
            let mut args = vec!["acp".into()];
            if let Some(effort) = reasoning_effort.filter(|e| !e.is_empty()) {
                args.push("--reasoning-effort".into());
                args.push(effort.to_owned());
            }
            Ok(AcpLaunch { args, env: Vec::new() })
        }
    }
}

// `start_local()` in `driver/mod.rs`: add to the AcpDriver arm:
ProviderKind::Cursor | ProviderKind::Fx | ProviderKind::Grok | ProviderKind::Kimi | ProviderKind::MyAgent => {
    Arc::new(acp::AcpDriver::start(provider, options, events)?)
}
*/

 // 3. `crates/padu-core/src/model_catalog.rs`
/*
pub fn fallback_models(provider: ProviderKind) -> Vec<ProviderModel> {
    match provider {
        // ...
        ProviderKind::MyAgent => vec![
            ProviderModel::new("default", "Default Model").default(),
            ProviderModel::new("fast", "Fast Model"),
        ],
    }
}
*/
