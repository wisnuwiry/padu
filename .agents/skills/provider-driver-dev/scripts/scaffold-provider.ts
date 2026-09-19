#!/usr/bin/env bun

/**
 * Provider Scaffolding Tool
 *
 * Scaffolds code additions for connecting a new AI coding agent to Padu.
 *
 * Usage:
 *   bun .agents/skills/provider-driver-dev/scripts/scaffold-provider.ts <id> "<DisplayName>" <binary> [--acp | --native]
 *
 * Example:
 *   bun .agents/skills/provider-driver-dev/scripts/scaffold-provider.ts cline "Cline" cline --acp
 *   bun .agents/skills/provider-driver-dev/scripts/scaffold-provider.ts goose "Goose" goose --acp
 */

import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../../../..");

function printUsage() {
  console.log(`
Usage:
  bun scaffold-provider.ts <id> "<DisplayName>" <binary> [--acp | --native]

Arguments:
  id           Unique lowercase alphanumeric identifier (e.g. 'cline', 'goose', 'gemini')
  DisplayName  Human-readable display title (e.g. 'Cline', 'Goose Agent', 'Gemini CLI')
  binary       Executable command name to detect in PATH (e.g. 'cline', 'goose', 'gemini')
  --acp        Configure as Agent Client Protocol provider (Recommended, default)
  --native     Configure as a native custom stdio driver

Example:
  bun scaffold-provider.ts goose "Goose" goose --acp
`);
}

const args = process.argv.slice(2);
if (args.length < 3 || args.includes("-h") || args.includes("--help")) {
  printUsage();
  process.exit(args.includes("-h") || args.includes("--help") ? 0 : 1);
}

const id = args[0].toLowerCase().replace(/[^a-z0-9_]/g, "");
const displayName = args[1];
const binary = args[2];
const isNative = args.includes("--native");
const mode = isNative ? "Native" : "ACP";

// PascalCase variant for Rust enum
const pascalCase = id
  .split(/[_-]/)
  .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
  .join("");

console.log(`\n================================================================`);
console.log(`  Scaffolding ${mode} Provider: ${displayName} (${pascalCase})`);
console.log(`================================================================\n`);

console.log(`Step 1: crates/padu-protocol/src/model.rs`);
console.log(`-----------------------------------------`);
console.log(`Add '${pascalCase}' to ProviderKind enum:

    #[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize, TS)]
    #[serde(rename_all = "camelCase")]
    pub enum ProviderKind {
        // ...
        ${pascalCase},
    }

In 'impl ProviderKind':
- Append 'Self::${pascalCase}' to 'pub const ALL'
- In 'id(self)':           Self::${pascalCase} => "${id}",
- In 'display_name(self)': Self::${pascalCase} => "${displayName}",
- In 'short_name(self)':   Self::${pascalCase} => "${displayName}",
- In 'command(self)':      Self::${pascalCase} => "${binary}",
`);

if (!isNative) {
  console.log(`Step 2: crates/padu-core/src/driver/acp.rs`);
  console.log(`-----------------------------------------`);
  console.log(`In 'fn launch_for(provider: ProviderKind, ...)':

    ProviderKind::${pascalCase} => Ok(AcpLaunch {
        args: vec!["acp".into()],
        env: Vec::new(),
    }),

In 'start_local(...)' in 'crates/padu-core/src/driver/mod.rs':
Add 'ProviderKind::${pascalCase}' to the AcpDriver match arm:
    ProviderKind::Cursor | ProviderKind::Fx | ProviderKind::Grok | ProviderKind::Kimi | ProviderKind::${pascalCase} => {
        Arc::new(acp::AcpDriver::start(provider, options, events)?)
    }
`);
} else {
  console.log(`Step 2: crates/padu-core/src/driver/${id}.rs`);
  console.log(`-----------------------------------------`);
  console.log(`Copy template from '.agents/skills/provider-driver-dev/examples/sample-native-driver.rs'`);
  console.log(`Register module in 'crates/padu-core/src/driver/mod.rs':
    mod ${id};
  And in 'start_local(...)':
    ProviderKind::${pascalCase} => Arc::new(${id}::${pascalCase}Driver::start(options, events)?),
`);
}

console.log(`Step 3: crates/padu-core/src/model_catalog.rs`);
console.log(`---------------------------------------------`);
console.log(`In 'pub fn fallback_models(provider: ProviderKind)':

    ProviderKind::${pascalCase} => vec![
        ProviderModel::new("default", tr!("model_option.default")).default(),
    ],
`);

console.log(`Step 4: Regenerate Protocol Types & Verify Parity`);
console.log(`-------------------------------------------------`);
console.log(`Run:
    bun run protocol:generate
    bun run protocol:check
`);

console.log(`Step 5: Test using the Provider Testing CLI`);
console.log(`-------------------------------------------`);
console.log(`Run:
    bun run provider:test probe ${id}
    bun run provider:test models ${id}
    bun run provider:test connect ${id}
    bun run provider:test turn ${id} "Reply with PONG"
`);
console.log(`Done!\n`);
