# hm — Agent Runtime Manager

A single Rust binary that detects, manages, and launches AI coding agent runtimes from one place.

You use Claude Code, Codex CLI, OpenCode, and a dozen other AI coding agents. Each has its own config files, auth tokens, proxy settings, and environment variables. They conflict. They shadow each other. `hm` fixes that.

## What it does

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

## Philosophy

**Inject, don't own.** `hm` doesn't generate or own your runtime configs. It detects what's already there, shows you the state, and injects proxy/endpoint/auth values on top — preferring ephemeral env injection over persistent file mutation.

- **Detect** all installed AI coding runtimes and their auth status
- **Inject** proxy/endpoint/API keys via environment at launch time (no files touched)
- **Launch** any runtime through `hm use` with a clean, isolated environment
- **Delegate** auth to native CLIs — `hm` never refreshes tokens itself

## Supported Runtimes

4 runtimes supported out of the box. Adding a new one is a single data entry in `registry.rs`.

| Runtime | Detection | Auth | Injection |
|---|---|---|---|
| Claude Code | `claude` binary + `~/.claude/` | OAuth + Keychain + env | `ANTHROPIC_BASE_URL` + `ANTHROPIC_API_KEY` |
| Codex CLI | `codex` binary + `~/.codex/` | ChatGPT OAuth + env | `OPENAI_BASE_URL` + `OPENAI_API_KEY` |
| OpenCode | `opencode` binary + `~/.config/opencode/` | Provider auth + env | `OPENAI_BASE_URL` + `OPENAI_API_KEY` |
| Pi | `pi` binary + `~/.pi/` | Token file | - |

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

## Usage

### Detect installed runtimes

```bash
hm detect
```

### Check auth status

```bash
hm auth status
```

Shows per-runtime auth details (OAuth tokens, API keys, expiry) and all AI-related environment variables.

### Login to a runtime

```bash
hm auth login codex    # delegates to `codex auth login`
hm auth login claude   # delegates to `claude` auth flow
hm auth login opencode # prints instructions
```

### Launch with proxy injection

```bash
# Preview what would be injected (dry-run)
hm inject plan codex --profile proxy

# Launch with injection
hm use codex --profile proxy
hm use claude --profile proxy

# Launch without injection (passthrough)
hm use codex
```

When launching with a profile, `hm`:
1. Strips all AI-related env vars (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, etc.)
2. Injects the profile's endpoint and API key for the target runtime
3. Injects proxy settings if configured
4. `exec`-replaces into the runtime binary (hm disappears)

### Pass extra arguments

```bash
hm use codex --profile proxy -- --model gpt-5.5
hm use claude --profile proxy -- --model claude-sonnet-4-20250514
```

## Harnesses

Harnesses are wrapper or extension tools that sit on top of runtimes. Builtins and user plugins are both loaded as declarative TOML manifests, then validated before any install, update, launch, inject, or remove side effect runs. Each harness gets its own isolated `$HOME` under `$HM/runtimes/<harness-id>/home`.

```bash
hm harness list
hm harness install <harness-id>
hm harness update <harness-id>
hm harness remove <harness-id>
hm harness remove <harness-id> --purge
hm use <harness-id> --profile proxy
hm <harness-id> -- --help
```

`hm harness list` discovers bundled manifests plus user manifests from `$XDG_CONFIG_HOME/hm/harnesses.d/*.toml`, `$XDG_DATA_HOME/hm/harnesses.d/*.toml`, and `$XDG_DATA_HOME/hm/plugins/*/harness.toml`. User manifests cannot override bundled IDs; duplicates fail closed before side effects.

Manifest authoring details, schema fields, `package.kind` values, and a complete `demo.toml` are documented in [docs/harness-manifest.md](docs/harness-manifest.md). The manifest model is declarative only: no shell snippets, no path launch binaries, no secret static env keys, and no seed files outside the isolation tree.

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
description = "Direct API access (no proxy)"
```

### Secret references

Secrets are never stored in config files. Use `secret://` URIs:

```
secret://file:///path/to/file        # read from file
secret://env/VAR_NAME                # read from env var
secret://keychain/service-name       # macOS Keychain lookup
```

## How injection works

Each runtime has an `InjectionSpec` that defines:
- Which env var receives the endpoint (`ANTHROPIC_BASE_URL`, `OPENAI_BASE_URL`, etc.)
- Which env var receives the API key
- Which env vars to strip before launch
- Whether the SDK auto-appends `/v1` (Claude does, OpenAI doesn't)

```
Profile (config.toml)          Runtime (registry.rs)
┌─────────────────────┐        ┌──────────────────────┐
│ endpoint: proxy.com │───────>│ ANTHROPIC_BASE_URL   │  (Claude)
│ bearer: secret://.. │───────>│ ANTHROPIC_API_KEY    │
└─────────────────────┘        │ strip: OPENAI_*      │
                               └──────────────────────┘
                               ┌──────────────────────┐
                        ┌─────>│ OPENAI_BASE_URL      │  (Codex)
                        │      │ OPENAI_API_KEY        │
                        │      │ strip: ANTHROPIC_*    │
                        │      └──────────────────────┘
```

## Adding a new runtime

Add a `RuntimeSpec` entry in `src/runtimes/registry.rs`:

```rust
RuntimeSpec {
    name: "My Runtime",
    binary_names: &["myrt"],
    version_arg: "--version",
    config_locator: ConfigLocator::EnvOrHome {
        env_var: "MYRT_HOME",
        home_relative: ".myrt",
    },
    config_files: &["config.toml"],
    auth_probes: &[
        AuthProbe::EnvKeys {
            vars: &["MYRT_API_KEY"],
            label: "API key",
        },
    ],
    injection: Some(&InjectionSpec {
        endpoint_env: "MYRT_BASE_URL",
        api_key_env: "MYRT_API_KEY",
        proxy_envs: &["HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY"],
        strip_envs: &["MYRT_API_KEY", "MYRT_BASE_URL"],
        endpoint_strip_v1: false,
    }),
},
```

No new files needed. Rebuild and the runtime is detected, authenticated, injectable, and launchable.

## Architecture

```
src/
  main.rs              CLI routing
  cli/mod.rs           clap command definitions
  runtimes/
    types.rs           RuntimeSpec, InjectionSpec, AuthProbe, AuthStatus
    registry.rs        13 runtime definitions (data, not code)
    auth.rs            auth probe engine (env, JSON, OAuth/JWT, keychain)
    mod.rs             detection engine (binary, version, config, auth)
  detect/mod.rs        `hm detect` table rendering
  auth/
    mod.rs             `hm auth status` detailed view
    login.rs           `hm auth login` native delegation
  config/mod.rs        profile config parsing + secret resolution
  secrets/mod.rs       secret:// URI resolver
  launch/mod.rs        `hm use` env sanitization + exec-replace
  inject/mod.rs        `hm inject plan` dry-run
```

## License

MIT
