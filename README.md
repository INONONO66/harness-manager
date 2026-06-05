# hm - Agent Runtime Manager

One command layer for AI coding agents, proxy profiles, auth state, and harness isolation.

Claude Code, Codex CLI, OpenCode, Pi, and harnesses built on top of them all want to own the same machine. They read the same env vars, write the same config folders, cache credentials in different places, and leak state across sessions. `hm` gives each tool a clean launch boundary without forcing you to abandon the native CLIs.

```bash
hm detect
hm use codex --profile proxy
hm use claude --profile proxy
hm harness install my-harness
hm use my-harness -- --help
```

## Why hm

- See every installed agent runtime and auth source in one table.
- Launch agents with clean, profile-driven env injection.
- Keep host secrets out of child processes unless a profile explicitly injects them.
- Run wrapper harnesses in isolated homes under `$XDG_DATA_HOME/hm/runtimes`.
- Add new harnesses with TOML manifests. No Rust edit, no core rebuild, no hardcoded harness IDs.
- Fail closed before install, update, remove, launch, or inject side effects when any manifest is invalid.

## What It Looks Like

```bash
$ hm detect
╭────────────────┬───────────┬───────────────────────┬──────────────────────────────────────────────────────────────╮
│ Runtime        │ Status    │ Version               │ Auth                                                         │
├────────────────┼───────────┼───────────────────────┼──────────────────────────────────────────────────────────────┤
│ Claude Code    │ Installed │ 2.1.152 (Claude Code) │ OAuth + OAuth (macOS Keychain) + API key (ANTHROPIC_API_KEY) │
│ Codex CLI      │ Installed │ codex-cli 0.136.0     │ ChatGPT OAuth + API key (OPENAI_API_KEY)                     │
│ OpenCode       │ Installed │ 1.15.13               │ Provider auth (7 providers) + API key (ANTHROPIC_API_KEY)    │
│ Pi             │ Not found │ -                     │ Not configured                                               │
╰────────────────┴───────────┴───────────────────────┴──────────────────────────────────────────────────────────────╯
```

```bash
$ hm harness list
# bundled and user/plugin harness manifests are loaded through the same registry
```

## Install

```bash
cargo install --path .
```

Or build from source:

```bash
git clone https://github.com/INONONO66/harness-manager.git
cd harness-manager
cargo build --release
cp target/release/hm ~/.local/bin/
```

## Daily Commands

```bash
# Inventory
hm detect
hm auth status

# Native login delegation
hm auth login codex
hm auth login claude

# Preview injected endpoint, bearer, and proxy env
hm inject plan codex --profile proxy

# Launch a runtime with a clean profile env
hm use codex --profile proxy
hm use claude --profile proxy

# Pass args through to the native CLI
hm use codex --profile proxy -- --model gpt-5.5
```

When `hm use` launches a target, `hm` strips hostile inherited AI env vars, resolves the selected profile, injects only the runtime-specific endpoint/API key/proxy variables, prepares the isolated home when needed, then `exec`s into the native binary.

## Harnesses

Harnesses are wrappers or extensions that sit on top of runtimes. Builtins and user plugins are declarative TOML manifests loaded by the same registry path. The core does not need to know names like a specific wrapper package or custom harness command.

```bash
hm harness list
hm harness install <harness-id>
hm harness update <harness-id>
hm harness remove <harness-id>
hm harness remove <harness-id> --purge
hm use <harness-id> --profile proxy
hm <harness-id> -- --help
```

Drop a manifest into one of these locations:

```text
$XDG_CONFIG_HOME/hm/harnesses.d/*.toml
$XDG_DATA_HOME/hm/harnesses.d/*.toml
$XDG_DATA_HOME/hm/plugins/*/harness.toml
~/.config/hm/harnesses.d/*.toml
~/.local/share/hm/harnesses.d/*.toml
~/.local/share/hm/plugins/*/harness.toml
```

Minimal manifest:

```toml
schema_version = 1
id = "my-harness"
aliases = ["mh"]
display_name = "My Harness"
target_runtime = "Codex CLI"
detect_binaries = ["my-harness"]
launch_binary = "my-harness"

[package]
kind = "npm-global"
package = "my-harness-package"

[isolation]
spoof_home = true
home_subdirs = []
static_envs = { CODEX_HOME = "{runtime_home}/.codex" }
```

Then run:

```bash
hm harness list
hm harness install my-harness
hm use my-harness -- --help
hm mh -- --help
```

Full schema and plugin packaging guidance: [docs/harness-manifest.md](docs/harness-manifest.md).

## Isolation Model

`hm` treats harness manifests as untrusted input and validates the complete registry before side effects. Invalid manifests block the operation before package managers run, launch envs are built, files are seeded, or isolation directories are removed.

- Duplicate harness IDs fail closed. User manifests cannot override bundled IDs.
- Harness IDs cannot shadow runtime commands such as `codex`.
- Launch binaries must be executable names, not paths or shell snippets.
- Package install strategies are structured: `npm-global`, `npx-installer`, `bunx-installer`, `python-tool`, or `manual`.
- Static env keys cannot be host secrets such as `*_TOKEN`, `*_SECRET`, or `*_API_KEY`.
- Seed files must live under `{home}/`, `{runtime_home}/`, `{state}/`, or `{tmp}/`; static envs may also use `{runtime_home}`, `{runtime_state}`, and `{runtime_logs}` for target-runtime shared state/log paths. Harness-specific plugin state belongs under `{home}` or `{state}`; runtime session DBs, auth, MCP config, and runtime plugins should point at `{runtime_home}`.
- Side-effecting operations take a per-target-runtime lock under `$XDG_DATA_HOME/hm/runtimes/.locks`.

## Configuration

Create `~/.config/hm/config.toml`:

```toml
default_profile = "proxy"

[profiles.proxy]
description = "Route through proxy gateway"

[profiles.proxy.llm]
endpoint = "https://your-proxy.example.com/v1"
bearer = "secret://file:///path/to/bearer-token"

[profiles.proxy.network]
http_proxy = "http://127.0.0.1:3128"
https_proxy = "http://127.0.0.1:3128"
no_proxy = "localhost,127.0.0.1"

[profiles.direct]
description = "Direct API access"
```

Secret references keep credentials out of config files:

```text
secret://file:///path/to/file
secret://env/VAR_NAME
secret://keychain/service-name
```

## Runtime Support

Runtime detection and injection are still native to the core because runtimes define auth probing and endpoint semantics.

| Runtime | Detection | Auth | Injection |
|---|---|---|---|
| Claude Code | `claude` binary + `~/.claude/` | OAuth + Keychain + env | `ANTHROPIC_BASE_URL` + `ANTHROPIC_API_KEY` |
| Codex CLI | `codex` binary + `~/.codex/` | ChatGPT OAuth + env | `OPENAI_BASE_URL` + `OPENAI_API_KEY` |
| OpenCode | `opencode` binary + `~/.config/opencode/` | Provider auth + env | `OPENAI_BASE_URL` + `OPENAI_API_KEY` |
| Pi | `pi` binary + `~/.pi/` | Token file | - |

## Architecture

```text
src/
  main.rs              CLI routing
  cli/mod.rs           clap command definitions
  runtimes/            runtime detection, auth probing, and injection specs
  harnesses/           manifest parser, registry, package commands, install flow
  isolation/           isolated env, seed files, path safety, locks
  config/              profile config parsing and secret references
  launch/              hm use target resolution and exec
  inject/              hm inject plan dry-run

harnesses/builtin/     bundled harness TOML manifests
docs/                  manifest authoring guide
```

## License

MIT
