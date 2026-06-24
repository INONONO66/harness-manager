# hm - Agent Runtime Manager

One command layer for AI coding agents, proxy profiles, auth state, and harness isolation.

Claude Code, Codex CLI, Gajae-Code, Grok CLI, OpenCode, Pi, and harnesses built on top of them all want to own the same machine. They read the same env vars, write the same config folders, cache credentials in different places, and leak state across sessions. `hm` gives each tool a clean launch boundary without forcing you to abandon the native CLIs.

```bash
hm detect
hm use codex --profile proxy
hm use claude --profile proxy
hm harness install lazycodex
hm use lazycodex -- --help
```

## Why hm

- See every installed agent runtime and auth source in one table.
- Launch agents with clean, profile-driven env injection.
- Keep host secrets out of child processes unless a profile explicitly injects them.
- Run wrapper harnesses in isolated homes under `$XDG_DATA_HOME/hm/runtimes`.
- Add new harnesses as native Rust definitions — one folder, one line in `defs::all()`. No hardcoded harness IDs in the engine.

## What It Looks Like

```bash
$ hm detect
╭────────────────┬───────────┬───────────────────────┬──────────────────────────────────────────────────────────────╮
│ Runtime        │ Status    │ Version               │ Auth                                                         │
├────────────────┼───────────┼───────────────────────┼──────────────────────────────────────────────────────────────┤
│ Claude Code    │ Installed │ 2.1.152 (Claude Code) │ OAuth + OAuth (macOS Keychain) + API key (ANTHROPIC_API_KEY) │
│ Codex CLI      │ Installed │ codex-cli 0.136.0     │ ChatGPT OAuth + API key (OPENAI_API_KEY)                     │
│ Gajae-Code     │ Installed │ gjc 0.4.4             │ Provider API key (ANTHROPIC_API_KEY)                         │
│ Grok CLI       │ Installed │ grok 1.1.7            │ API key (GROK_API_KEY)                                       │
│ OpenCode       │ Installed │ 1.15.13               │ Provider auth (7 providers) + API key (ANTHROPIC_API_KEY)    │
│ Pi             │ Not found │ -                     │ Not configured                                               │
╰────────────────┴───────────┴───────────────────────┴──────────────────────────────────────────────────────────────╯
```

```bash
$ hm harness list
# nine built-in harnesses loaded from native Rust definitions
```

## Install

Supported platforms: macOS and Linux. Windows is not supported because `hm`
uses Unix process exec semantics and Unix filesystem permissions for launch,
isolation, and secret handling.

```bash
curl -fsSL https://raw.githubusercontent.com/INONONO66/harness-manager/main/scripts/install.sh | sh
```

Or install the npm wrapper package:

```bash
npm install -g harness-manager
```

The npm package downloads the matching GitHub Release binary during install.

Install and then install all built-in harnesses in one step:

```bash
curl -fsSL https://raw.githubusercontent.com/INONONO66/harness-manager/main/scripts/install.sh | sh -s -- --install-harnesses
```

Or build from source:

```bash
git clone https://github.com/INONONO66/harness-manager.git
cd harness-manager
cargo build --release
cp target/release/hm ~/.local/bin/
```

For local development, install from the checkout:

```bash
cargo install --path .
```

## First-Time Bootstrap (`hm init`)

Runtimes and harnesses are compiled into the `hm` binary — there are no manifest files to copy. `hm init --install` installs every non-manual built-in harness package.

```bash
hm init --install      # install every non-manual built-in harness
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

Harnesses are wrappers or extensions that sit on top of runtimes. Like runtimes, they are native Rust definitions in `src/harnesses/defs/` — one folder per harness, split by concern:

- `mod.rs` — identity: id, aliases, display name, target runtime, detect binaries, launch binary
- `package.rs` — install/remove/update strategy (npm-global, npm-isolated, npx-installer, bunx-installer, python-tool, custom, git-worktree)
- `isolation.rs` — isolation env: static env vars, home subdirs, seed files, caveat

Injection is inherited from each harness's target runtime. Adding a harness is a Rust code change — one folder plus one line in `defs::all()`. No hardcoded harness IDs appear in the engine.

Nine built-in harnesses:

| id | aliases | target runtime |
|---|---|---|
| lazycodex | lc | Codex CLI |
| omx | — | Codex CLI |
| superpowers | sp | Codex CLI |
| gstack | gs | Codex CLI |
| ouroboros | — | Codex CLI |
| gstack-claude | gstack-cc, gsc | Claude Code |
| superpowers-claude | superpowers-cc, spc | Claude Code |
| omo | — | OpenCode |
| omc | — | Claude Code |

```bash
hm harness list
hm harness install <id>
hm harness update <id>
hm harness remove <id>
hm harness remove <id> --purge
hm use <id> --profile proxy
hm <id> -- --help
```

## Isolation Model

- The `npm-isolated` package kind installs into the harness isolation home (`$XDG_DATA_HOME/hm/runtimes/<id>/home/.npm`) via `NPM_CONFIG_PREFIX` so the package's binaries never appear on the host `PATH`; `hm use <harness>` adds the declared package bin dir to the launch `PATH` and exec's the binary directly. Use this for harnesses whose CLI you want gated behind `hm use`.
- hm links session/transcript artifacts from the user's native runtime home into the isolated runtime home. Bundled policies cover Codex sessions/history, OpenCode session stores and `opencode.db*`, Claude projects/transcripts, Pi sessions, Gajae session DBs, and Grok sessions.
- Bundled runtimes do not share host auth files. Profile launches use profile-driven gateway/API credentials; non-profile launches keep auth isolated.
- Package-manager fallback choices are recorded after install and preferred for later update/remove, so `uv`/`pipx`/`pip` and `bunx`/`npx` paths do not drift silently between lifecycle commands.
- Side-effecting operations take a per-harness runtime lock under `$XDG_DATA_HOME/hm/runtimes/.locks`.

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

## npm Publishing

The npm package is published through GitHub Actions trusted publishing. Do not add `NPM_TOKEN` or `NODE_AUTH_TOKEN` to the publish job; npm uses GitHub OIDC with `id-token: write`.

The first-ever npm package version cannot be created by OIDC because npm only allows trusted publisher configuration after the package exists. Create the package once, then configure npm Trusted Publisher for GitHub Actions:

- Owner/repository: `INONONO66/harness-manager`
- Workflow filename: `npm-publish.yml`
- Allowed action: `npm publish`

After that one-time setup, publishing a GitHub tag runs `.github/workflows/release.yml`; after that workflow succeeds, `.github/workflows/npm-publish.yml` runs automatically, verifies the tag matches `package.json`, installs the latest npm CLI, checks that token env vars are absent, validates the package, and runs `npm publish --access public`.

## Runtime Support

Runtimes are native Rust definitions in `src/runtimes/defs/` — one file per runtime, each a `pub fn record() -> RuntimeRecord`, aggregated by `defs::all()`. There is no runtime TOML manifest layer and no user/plugin runtime discovery; adding or changing a runtime is a code change. Harnesses follow the same pattern — native Rust definitions in `src/harnesses/defs/` (see Harnesses above).

Each runtime declares one of three injection strategies (the `InjectionRecord` enum — the only strategies in core) plus a containment mode (`RuntimeRecord.spoof_home`).

| Runtime | Detection | Auth | Strategy | Injection |
|---|---|---|---|---|
| Claude Code | `claude` binary + `~/.claude/` | OAuth + Keychain + env | `env` | `ANTHROPIC_BASE_URL` + `ANTHROPIC_API_KEY` (with `/v1` stripped) |
| Codex CLI | `codex` binary + `~/.codex/` | ChatGPT OAuth + env | `codex-config-seed` | writes top-level `openai_base_url` + `model_provider` to `~/.codex/config.toml` (merging with existing seed_files content) and injects `CODEX_API_KEY` env (codex 0.137 reads this at runtime, not `OPENAI_API_KEY`) |
| Gajae-Code | `gjc` binary + `~/.gjc/agent/` | Provider env + auth broker env | `provider-config-seed` | seeds `~/.gjc/agent/models.yml` `providers.<id>.{baseUrl,apiKey,headers}` for every gateway provider |
| Grok CLI | `grok` binary + `~/.grok/` | `GROK_API_KEY` env + `user-settings.json` | `env` | `GROK_BASE_URL` + `GROK_API_KEY` for xAI/Grok profiles |
| OpenCode | `opencode` binary + `~/.config/opencode/` | Provider auth + env | `provider-config-seed` | seeds `~/.config/opencode/opencode.json` `provider.<id>.options.{baseURL,apiKey,headers}` for every gateway provider; falls back to `[profiles.X.llm]` as single-provider `openai` seed |
| Pi | `pi` binary + `~/.pi/agent/` | Token file | `provider-config-seed` | seeds `~/.pi/agent/models.json` `providers.<id>.{baseUrl,apiKey,headers}` for every gateway provider |

The three strategies are the only ones in core. Per-runtime knowledge lives in the runtime's `defs/<name>.rs` record. Picker tree:

- Endpoint goes in an env var, single provider per runtime → `env`
- Config file holds repeated provider sub-trees keyed by provider name → `provider-config-seed`
- Config file holds top-level keys (no per-provider table) AND auth comes from an env var → `codex-config-seed`

### Isolation: RedirectOnly vs SpoofHome

Each runtime declares a containment mode via `RuntimeRecord.spoof_home`, dispatched in `launch::build_launch_env` on the target runtime's `runtime.spoof_home` (harnesses inherit it from their target runtime):

- **RedirectOnly** (`spoof_home = false` — codex, opencode, pi, gajae-code, grok, and every harness that targets them): `HOME` stays the host's. Only the runtime's own state dir is redirected into the hm tree via its native env var (`CODEX_HOME`, `PI_CODING_AGENT_DIR`, `OPENCODE_CONFIG_DIR`, …). The child inherits the full host environment minus AI API keys, so host tooling — `gh`, `cargo`, `ssh`, mise/asdf — works exactly as it does outside hm. Containment still holds because the runtime writes its own config/sessions/auth under the redirected dir.
- **SpoofHome** (`spoof_home = true` — only Claude, plus harnesses that target Claude such as `gstack-claude`/`superpowers-claude`/`omc`): `HOME` is spoofed to an isolated tree and the child env is reduced to a safe allowlist with host secrets stripped. Used when a runtime ignores its own redirect env (Claude's `CLAUDE_CONFIG_DIR`). Harnesses inherit this from their target runtime.

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

[injection.provider_api_key_envs]
anthropic = "ANTHROPIC_API_KEY"
openai = "OPENAI_API_KEY"
google = "GOOGLE_API_KEY"

[injection.provider_header_overrides.anthropic]
"x-api-key" = "{bearer}"
"Authorization" = "Bearer {bearer}"
```

`hm use opencode --profile proxy` writes a JSON file under the isolation home (never the user's real `~`). The file is deep-merged into any existing user content. `legacy_provider` (optional) tells the strategy how to fall back to `[profiles.X.llm]`: hm seeds that one provider with `llm.endpoint` and `llm.bearer`.
`provider_api_key_envs` maps each supported provider id to the child-process env var that should receive the resolved bearer; missing mappings fail closed before env or file writes.

### Injection strategy 3: `codex-config-seed` (top-level TOML + env hybrid)

```toml
[injection]
strategy = "codex-config-seed"
config_path = "{home}/.codex/config.toml"
openai_base_url_key = "openai_base_url"
model_provider_key = "model_provider"
model_provider_value = "openai"
provider = "openai"
supported_providers = ["openai"]
api_key_env = "CODEX_API_KEY"
strip_envs = ["OPENAI_API_KEY", "OPENAI_BASE_URL", "CODEX_API_KEY", "CODEX_ACCESS_TOKEN"]
overwrite = false
endpoint_strip_v1 = false
```

`hm use codex --profile proxy` writes two top-level keys to the TOML config (via `toml_edit::DocumentMut`, preserving comments and existing keys from `[isolation.seed_files]`), then strips `strip_envs` from the launch env and sets `api_key_env` to the resolved bearer. The bearer NEVER reaches the file — only the env var. This strategy is single-provider only: the gateway must route the configured `provider` (or the legacy `[profiles.X.llm]` fallback is used).

Adding a new runtime is a Rust file under `src/runtimes/defs/` plus one line in `defs::all()` — no TOML, no codegen. The field shapes shown above as TOML now live as fields on the `InjectionRecord` variants of each runtime's `record()`.

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
  runtimes/            native runtime definitions, registry, sandboxed detection
    defs/                one Rust record() per runtime + all() (claude, codex, ...)
    manifest/records.rs  owned RuntimeRecord domain types
    registry/dynamic.rs  RuntimeRegistry::load (from defs::all())
    auth/                per-variant auth probe dispatch
  harnesses/           native harness definitions (defs/), registry, package commands, install flow
  isolation/           isolated env, seed files, path safety, locks
  config/              profile config parsing + gateway schema + secret references
  launch/
    injection.rs         the only place that knows env / provider-config-seed / codex-config-seed
    target.rs            runtime/harness resolution
    mod.rs               run_use and exec
  inject/mod.rs        hm inject plan dry-run (calls validate_provider_config_seed)

src/runtimes/defs/     native runtime records (claude, codex, gajae-code, grok, opencode, pi)
src/harnesses/defs/    native harness records (lazycodex, omx, superpowers, gstack, ouroboros, gstack-claude, superpowers-claude, omo, omc)
```

## License

MIT
