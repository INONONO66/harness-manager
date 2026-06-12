# hm - Agent Runtime Manager

One command layer for AI coding agents, proxy profiles, auth state, and harness isolation.

Claude Code, Codex CLI, Gajae-Code, Grok CLI, OpenCode, Pi, and harnesses built on top of them all want to own the same machine. They read the same env vars, write the same config folders, cache credentials in different places, and leak state across sessions. `hm` gives each tool a clean launch boundary without forcing you to abandon the native CLIs.

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
│ Gajae-Code     │ Installed │ gjc 0.4.4             │ Provider API key (ANTHROPIC_API_KEY)                         │
│ Grok CLI       │ Installed │ grok 1.1.7            │ API key (GROK_API_KEY)                                       │
│ OpenCode       │ Installed │ 1.15.13               │ Provider auth (7 providers) + API key (ANTHROPIC_API_KEY)    │
│ Pi             │ Not found │ -                     │ Not configured                                               │
╰────────────────┴───────────┴───────────────────────┴──────────────────────────────────────────────────────────────╯
```

```bash
$ hm harness list
# bundled and user/plugin harness manifests are loaded through the same registry
```

## Install

Supported platforms: macOS and Linux. Windows is not supported because `hm`
uses Unix process exec semantics and Unix filesystem permissions for launch,
isolation, and secret handling.

```bash
curl -fsSL https://raw.githubusercontent.com/INONONO66/harness-manager/main/scripts/install.sh | sh
```

Install and copy built-in manifests in one step:

```bash
curl -fsSL https://raw.githubusercontent.com/INONONO66/harness-manager/main/scripts/install.sh | sh -s -- --init
```

Install everything non-manual that `hm init --install` can manage:

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

`hm init` copies every built-in runtime and harness manifest into `~/.config/hm/` so you can edit them. The embedded copies in the binary stay as defaults; your edits take precedence.

```bash
hm init                # write 6 runtimes + 5 harnesses to ~/.config/hm/{runtimes,harnesses}.d/ (skip existing)
hm init --force        # overwrite existing user manifests with the embedded defaults
hm init --install      # also install every non-manual harness package
hm init --force --install   # clean reset: refresh manifests AND reinstall harnesses
```

**Override rule.** User runtime manifests override bundled runtimes by normalized display name OR binary name; user harness manifests override by id OR alias. Byte-identical `hm init` copies are silent; any divergence emits a `note:` on stderr. Shadowed builtin routes are preserved as lookup aliases on the replacement, so harnesses referencing the old display name still resolve. A single user manifest shadowing MULTIPLE builtins fails closed, and two user manifests sharing a route fail closed.

**What you can edit.** Anything in the manifest schema — change `supported_providers`, add `provider_api_key_envs` or `provider_header_overrides`, retarget `config_path`, swap the package install strategy, add new `auth_probes`, etc. Run `hm use <runtime>` and your changes drive the next launch.

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
hm harness install <git-url-or-path> --alias <harness-id>
hm harness install-package <package> --alias <harness-id> --runtime codex --kind npm-global --binary <bin>
hm harness add <git-url-or-path> --alias <harness-id>
hm harness update <harness-id>
hm harness remove <harness-id>
hm harness remove <harness-id> --purge
hm use <harness-id> --profile proxy
hm <harness-id> -- --help
```

Install directly from a plugin repository when it contains `harness.toml` at
the repository root:

```bash
hm harness install https://github.com/example/my-harness --alias my-harness
hm use my-harness -- --help
```

Or register it without installing yet:

```bash
hm harness add https://github.com/example/my-harness --alias my-harness
hm harness install my-harness
```

For simple package-backed harnesses, generate the manifest and install in one
step:

```bash
hm harness install-package my-harness-package \
  --alias my-harness \
  --runtime codex \
  --kind npm-global \
  --binary my-harness
```

`hm` stores the repository under the plugin discovery path and rewrites the
manifest command id to the alias you chose. The runtime shared-state policy
comes from the target runtime manifest; users do not edit `[shared_state]` in
harness manifests.

You can also drop a manifest into one of these locations:

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
self_update = "managed-by-hm"

[isolation]
spoof_home = true
home_subdirs = []
static_envs = { CODEX_HOME = "{home}/.codex" }
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
- Package install strategies are structured: `npm-global`, `npm-isolated`, `npx-installer`, `bunx-installer`, `python-tool`, `custom`, or `manual`. The `npm-isolated` kind installs into the harness isolation home (`$XDG_DATA_HOME/hm/runtimes/<id>/home/.npm`) via `NPM_CONFIG_PREFIX` so the package's binaries never appear on the host `PATH`; `hm use <harness>` adds the declared package bin dir to the launch `PATH` and exec's the binary directly. Use this for harnesses whose CLI you want gated behind `hm use`.
- Static env keys cannot be host secrets such as `*_TOKEN`, `*_SECRET`, or `*_API_KEY`.
- Seed files must live under `{home}/`, `{runtime_home}/`, `{state}/`, or `{tmp}/`; for harnesses, `{runtime_home}` resolves to the harness runtime root so runtime session DBs, auth, MCP config, plugins, hooks, prompts, and trust state stay isolated per harness.
- hm links known runtime database files back to the user's main runtime DBs: Codex `*.sqlite*` under `~/.codex`, and OpenCode `*.db*` under `~/.local/share/opencode`. Harness config/plugin files stay isolated, while conversation/log/memory DBs stay shared.
- Host auth files are shared only for isolated launches without a profile. When a profile is applied, profile-driven gateway/API credentials are used and host OAuth/auth files are not linked into the harness home.
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

## Runtime Support

Runtimes are declarative TOML manifests, not Rust statics. Built-ins live in `runtimes/builtin/*.toml` and are compiled into the binary; users drop additional manifests under:

```text
$XDG_CONFIG_HOME/hm/runtimes.d/*.toml
$XDG_DATA_HOME/hm/runtimes.d/*.toml
$XDG_DATA_HOME/hm/plugins/*/runtime.toml
```

Each manifest declares one of three injection strategies. `hm` validates the full registry before any side effect. User runtime manifests with the same normalized binary name or display name as a builtin override the builtin (with a `note:` on stderr unless the file is byte-identical to the embedded copy). A single user manifest that would shadow MULTIPLE builtins fails closed, and two user manifests sharing a route also fail closed.

| Runtime | Detection | Auth | Strategy | Injection |
|---|---|---|---|---|
| Claude Code | `claude` binary + `~/.claude/` | OAuth + Keychain + env | `env` | `ANTHROPIC_BASE_URL` + `ANTHROPIC_API_KEY` (with `/v1` stripped) |
| Codex CLI | `codex` binary + `~/.codex/` | ChatGPT OAuth + env | `codex-config-seed` | writes top-level `openai_base_url` + `model_provider` to `~/.codex/config.toml` (merging with existing seed_files content) and injects `CODEX_API_KEY` env (codex 0.137 reads this at runtime, not `OPENAI_API_KEY`) |
| Gajae-Code | `gjc` binary + `~/.gjc/agent/` | Provider env + auth broker env | `provider-config-seed` | seeds `~/.gjc/agent/models.yml` `providers.<id>.{baseUrl,apiKey,headers}` for every gateway provider |
| Grok CLI | `grok` binary + `~/.grok/` | `GROK_API_KEY` env + `user-settings.json` | `env` | `GROK_BASE_URL` + `GROK_API_KEY` for xAI/Grok profiles |
| OpenCode | `opencode` binary + `~/.config/opencode/` | Provider auth + env | `provider-config-seed` | seeds `~/.config/opencode/opencode.json` `provider.<id>.options.{baseURL,apiKey,headers}` for every gateway provider; falls back to `[profiles.X.llm]` as single-provider `openai` seed |
| Pi | `pi` binary + `~/.pi/agent/` | Token file | `provider-config-seed` | seeds `~/.pi/agent/models.json` `providers.<id>.{baseUrl,apiKey,headers}` for every gateway provider |

The three strategies are the only ones in core. Per-runtime knowledge lives in the manifest. Picker tree:

- Endpoint goes in an env var, single provider per runtime → `env`
- Config file holds repeated provider sub-trees keyed by provider name → `provider-config-seed`
- Config file holds top-level keys (no per-provider table) AND auth comes from an env var → `codex-config-seed`

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
    injection.rs         the only place that knows env / provider-config-seed / codex-config-seed
    target.rs            runtime/harness resolution
    mod.rs               run_use and exec
  inject/mod.rs        hm inject plan dry-run (calls validate_provider_config_seed)

runtimes/builtin/      bundled runtime TOML manifests (claude, codex, gajae-code, grok, opencode, pi)
harnesses/builtin/     bundled harness TOML manifests
docs/                  manifest authoring guide
```

## License

MIT
