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

- A user harness manifest with the same id or alias as a bundled harness overrides the builtin and prints `note:` to stderr. Byte-identical copies (e.g. fresh `hm init` output) are silent. A single user manifest that would shadow MULTIPLE bundled harnesses fails closed; two user manifests sharing a route also fail closed.
- Harness IDs cannot shadow runtime commands such as `codex`.
- Launch binaries must be executable names, not paths or shell snippets.
- Package install strategies are structured: `npm-global`, `npx-installer`, `bunx-installer`, `python-tool`, or `manual`.
- Static env keys cannot be host secrets such as `*_TOKEN`, `*_SECRET`, or `*_API_KEY`.
- Seed files must live under `{home}/`, `{runtime_home}/`, `{state}/`, or `{tmp}/`; static envs may also use `{runtime_home}`, `{runtime_state}`, and `{runtime_logs}` for target-runtime shared state/log paths. Harness-specific plugin state belongs under `{home}` or `{state}`; runtime session DBs, auth, MCP config, and runtime plugins should point at `{runtime_home}`.
- Side-effecting operations take a per-target-runtime lock under `$XDG_DATA_HOME/hm/runtimes/.locks`.

## Profiles And Proxy Gateway

Create `~/.config/hm/config.toml`. The recommended block for routing every provider through one gateway is `[profiles.<name>.gateway]`. Legacy `[profiles.<name>.llm]` still works as a single-provider fallback (see Runtime Support below).

```toml
default_profile = "proxy"

[profiles.proxy]
description = "Route Anthropic, OpenAI, and Google through one gateway"

[profiles.proxy.gateway]
base_url = "https://proxy.example.com/v1"
bearer = "secret://file:///path/to/bearer-token"
providers = ["anthropic", "openai", "google"]

[profiles.proxy.network]
http_proxy = "http://127.0.0.1:3128"
https_proxy = "http://127.0.0.1:3128"
no_proxy = "localhost,127.0.0.1"

[profiles.direct]
description = "Direct API access"

[profiles.legacy.llm]
endpoint = "https://your-proxy.example.com/v1"
bearer = "secret://file:///path/to/bearer-token"
```

Optional per-provider header overrides (for example to send both `x-api-key` and `Authorization` to a provider):

```toml
[profiles.proxy.gateway.provider_headers.anthropic]
"x-api-key" = "{bearer}"
"Authorization" = "Bearer {bearer}"
```

Secret references keep credentials out of config files:

```text
secret://file:///path/to/file
secret://env/VAR_NAME
secret://keychain/service-name
secret://hm/<secret-name>          # hm's own secret store
```

## Runtime Support

Runtimes are declarative TOML manifests, not Rust statics. Built-ins live in `runtimes/builtin/*.toml` and are compiled into the binary; users drop additional manifests under:

```text
$XDG_CONFIG_HOME/hm/runtimes.d/*.toml
$XDG_DATA_HOME/hm/runtimes.d/*.toml
$XDG_DATA_HOME/hm/plugins/*/runtime.toml
```

Each manifest declares one of two injection strategies. `hm` validates the full registry before any side effect. User runtime manifests with the same normalized binary name or display name as a builtin override the builtin (with a `note:` on stderr unless the file is byte-identical to the embedded copy). A single user manifest that would shadow MULTIPLE builtins fails closed, and two user manifests sharing a route also fail closed.

| Runtime | Detection | Auth | Strategy | Injection |
|---|---|---|---|---|
| Claude Code | `claude` binary + `~/.claude/` | OAuth + Keychain + env | `env` | `ANTHROPIC_BASE_URL` + `ANTHROPIC_API_KEY` (with `/v1` stripped) |
| Codex CLI | `codex` binary + `~/.codex/` | ChatGPT OAuth + env | `env` | `OPENAI_BASE_URL` + `OPENAI_API_KEY` |
| OpenCode | `opencode` binary + `~/.config/opencode/` | Provider auth + env | `provider-config-seed` | seeds `~/.config/opencode/opencode.json` `provider.<id>.options.{baseURL,apiKey,headers}` for every gateway provider; falls back to `[profiles.X.llm]` as single-provider `openai` seed |
| Pi | `pi` binary + `~/.pi/agent/` | Token file | `provider-config-seed` | seeds `~/.pi/agent/models.json` `providers.<id>.{baseUrl,apiKey,headers}` for every gateway provider |

The two strategies are the only ones in core. Per-runtime knowledge lives in the manifest.

### Injection strategy 1: `env` (single-provider runtimes)

```toml
[injection]
strategy = "env"
provider = "anthropic"
supported_providers = ["anthropic"]
endpoint_env = "ANTHROPIC_BASE_URL"
api_key_env = "ANTHROPIC_API_KEY"
strip_envs = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL"]
endpoint_strip_v1 = true
```

`hm use claude --profile proxy` strips `strip_envs` from the child env, then sets `endpoint_env` and `api_key_env` from the active profile's gateway. If no gateway is present and the legacy `[profiles.X.llm]` block is, hm falls back to that single endpoint/bearer.

### Injection strategy 2: `provider-config-seed` (multi-provider runtimes)

```toml
[injection]
strategy = "provider-config-seed"
config_path = "{home}/.config/opencode/opencode.json"
root_key = "provider"
provider_base_url_key = "options.baseURL"
provider_api_key_key = "options.apiKey"
provider_headers_key = "options.headers"
supported_providers = ["anthropic", "openai", "google", "openrouter", "groq", "xai", ...]
overwrite = false
endpoint_strip_v1 = false
legacy_provider = "openai"

[injection.provider_header_overrides.anthropic]
"x-api-key" = "{bearer}"
"Authorization" = "Bearer {bearer}"
```

`hm use opencode --profile proxy` writes a JSON file under the isolation home (never the user's real `~`). The file is deep-merged into any existing user content. `legacy_provider` (optional) tells the strategy how to fall back to `[profiles.X.llm]`: hm seeds that one provider with `llm.endpoint` and `llm.bearer`.

Adding a new runtime requires only a TOML file. No Rust change.

### Security

- Duplicate runtime routes (normalized binary names and display names) fail closed at registry load.
- `config_path` for seed strategy must live under `{home}/`.
- Seed file writes refuse to follow any symlink chain out of the isolation home.
- Existing seed JSON that fails to parse is preserved (never silently overwritten when `overwrite = false`).
- Header values are validated for CRLF/NUL/control chars BEFORE and AFTER `{bearer}` substitution.
- Static env keys cannot be host secrets such as `*_TOKEN`, `*_SECRET`, or `*_API_KEY`.
- Auth-probe paths (including keychain marker files) must be relative.

## Architecture

```text
src/
  main.rs              CLI routing
  cli/mod.rs           clap command definitions
  runtimes/            runtime TOML manifest parser, registry, sandboxed detection
    builtin.rs           built-in manifest index (codegen from build.rs)
    manifest.rs          owned types + validation + parse_toml
    registry/dynamic.rs  RuntimeRegistry::load (builtins + user/plugin discovery)
    auth.rs              per-variant auth probe dispatch
  harnesses/           harness manifest parser, registry, package commands, install flow
  isolation/           isolated env, seed files, path safety, locks
  config/              profile config parsing + gateway schema + secret references
  launch/
    injection.rs         the only place that knows env vs config-seed strategy
    target.rs            runtime/harness resolution
    mod.rs               run_use and exec
  inject/mod.rs        hm inject plan dry-run (calls validate_provider_config_seed)

runtimes/builtin/      bundled runtime TOML manifests (claude, codex, opencode, pi)
harnesses/builtin/     bundled harness TOML manifests
docs/                  manifest authoring guide
```

## License

MIT
