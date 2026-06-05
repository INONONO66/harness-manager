# Harness Manifest

Harnesses are declarative TOML manifests. The core binary loads bundled manifests and user/plugin manifests through the same registry path, then validates the full registry before any install, update, launch, inject, or remove side effect runs.

User manifests are discovered from:

- `$XDG_CONFIG_HOME/hm/harnesses.d/*.toml`
- `$XDG_DATA_HOME/hm/harnesses.d/*.toml`
- `$XDG_DATA_HOME/hm/plugins/*/harness.toml`
- `~/.config/hm/harnesses.d/*.toml` when `XDG_CONFIG_HOME` is not set
- `~/.local/share/hm/harnesses.d/*.toml` and `~/.local/share/hm/plugins/*/harness.toml` when `XDG_DATA_HOME` is not set

User manifests cannot override bundled harness IDs. Duplicate IDs fail closed before command side effects.

```toml
schema_version = 1
id = "demo"
display_name = "Demo Harness"
target_runtime = "Codex CLI"
detect_binaries = ["demo-agent"]
launch_binary = "demo-agent"
launch_args = ["agent"]

[package]
kind = "manual"
instructions = "Install demo-agent from your plugin distribution."

[isolation]
subdir = "demo"
spoof_home = true
home_subdirs = [".codex"]
static_envs = { CODEX_HOME = "{home}/.codex", DEMO_STATE = "{state}/demo" }
caveat = "Demo harness runs with an isolated HOME."

[[isolation.seed_files]]
path = "{home}/.codex/config.toml"
content = "analytics_enabled = false\n"
overwrite = false
```

## Fields

`schema_version`: must be `1`.

`id`: command identifier. It must be lowercase ASCII, 2-64 characters, and may contain digits, `_`, and `-`.

`display_name`: label shown in `hm harness list` and launch output.

`target_runtime`: existing runtime display name, such as `Codex CLI`, `Claude Code`, or `OpenCode`.

`detect_binaries`: one or more executable names used to report install status.

`launch_binary`: optional executable name to run instead of the target runtime binary.

`launch_args`: optional fixed arguments prepended before user arguments.

`package.kind`: one of `npm-global`, `npx-installer`, `bunx-installer`, `python-tool`, or `manual`.

Package payload by kind:

- `npm-global`: `package`
- `npx-installer`: `package`, optional `args`
- `bunx-installer`: `package`, optional `args`
- `python-tool`: `package`
- `manual`: `instructions`

`isolation.subdir`: optional runtime directory name. Defaults to `id`.

`isolation.spoof_home`: when true, `HOME` points at the harness isolation home.

`isolation.home_subdirs`: directories to create under the isolated home.

`isolation.static_envs`: static environment values with `{home}`, `{state}`, and `{tmp}` token substitution.

`isolation.seed_files`: files created inside the isolation tree before launch or package-manager work.

## Security Model

Manifests are declarative only. They cannot run shell snippets, set secret env keys, point launch binaries at paths, or seed files outside the isolation tree. Paths are canonicalized during discovery; symlinked manifests and plugin directories are rejected. Each side-effecting operation takes a per-harness lock under `$XDG_DATA_HOME/hm/runtimes/.locks/` so launch setup and removal do not interleave.
