# Harness Manifest Guide

Ship a new harness without touching Rust core.

A harness is a declarative TOML file that tells `hm` how to install, detect, isolate, and launch a wrapper tool on top of an existing runtime. The same registry loads bundled manifests and user/plugin manifests, validates all of them, then allows side-effecting commands to run.

## Put The File Here

For a local machine:

```bash
mkdir -p ~/.config/hm/harnesses.d
$EDITOR ~/.config/hm/harnesses.d/my-harness.toml
```

For a plugin package:

```bash
mkdir -p ~/.local/share/hm/plugins/my-plugin
$EDITOR ~/.local/share/hm/plugins/my-plugin/harness.toml
```

For a git repository that already ships `harness.toml` at its root:

```bash
hm harness install https://github.com/example/my-plugin --alias my-plugin
```

To only register the source and install later:

```bash
hm harness add https://github.com/example/my-plugin --alias my-plugin
hm harness install my-plugin
```

For package-backed harnesses that do not ship a repository manifest yet:

```bash
hm harness install-package my-plugin-package \
  --alias my-plugin \
  --runtime codex \
  --kind npm-global \
  --binary my-plugin
```

Discovery paths:

```text
$XDG_CONFIG_HOME/hm/harnesses.d/*.toml
$XDG_DATA_HOME/hm/harnesses.d/*.toml
$XDG_DATA_HOME/hm/plugins/*/harness.toml
~/.config/hm/harnesses.d/*.toml
~/.local/share/hm/harnesses.d/*.toml
~/.local/share/hm/plugins/*/harness.toml
```

If a manifest is malformed, `hm` fails before package-manager, launch, inject, remove, or purge side effects.

## Copy This First

```toml
schema_version = 1
id = "demo"
aliases = ["dm"]
display_name = "Demo Harness"
target_runtime = "Codex CLI"
detect_binaries = ["demo-agent"]
launch_binary = "demo-agent"
launch_args = ["agent"]

[package]
kind = "manual"
instructions = "Install demo-agent from your plugin distribution."
self_update = "not-applicable"

[isolation]
subdir = "demo"
home_subdirs = []
static_envs = { CODEX_HOME = "{home}/.codex", DEMO_STATE = "{state}/demo" }
caveat = "Demo harness runs with an isolated HOME."

[[isolation.seed_files]]
path = "{home}/.codex/config.toml"
content = "analytics_enabled = false\n"
overwrite = false
```

Check it:

```bash
hm harness list
hm harness install demo
hm use demo -- --help
hm dm -- --help
hm harness remove demo --purge
```

## Choose An Install Strategy

Use one structured `package.kind`. `hm` never runs shell snippets from manifests.

```toml
[package]
kind = "npm-global"
package = "my-package"
self_update = "managed-by-hm"
```

```toml
[package]
kind = "npx-installer"
package = "my-package"
args = ["setup"]
self_update = "managed-by-hm"
```

```toml
[package]
kind = "bunx-installer"
package = "my-package"
args = ["install"]
self_update = "managed-by-hm"
```

```toml
[package]
kind = "python-tool"
package = "my-package"
self_update = "managed-by-hm"
```

```toml
[package]
kind = "custom"
install = ["installer", "install", "my-package"]
update = ["installer", "upgrade", "my-package"]
uninstall = ["installer", "remove", "my-package"]
bin_subdir = ".custom/bin"
self_update = "managed-by-hm"
```

```toml
[package]
kind = "manual"
instructions = "Install the binary with your plugin manager."
self_update = "not-applicable"
```

## Field Reference

`schema_version`: must be `1`.

`id`: command identifier. Lowercase ASCII, 2-64 characters, with digits, `_`, and `-` allowed. It must not duplicate a bundled/user harness ID or shadow a runtime command.

`aliases`: optional short command identifiers, validated like `id`. Aliases must not duplicate any harness ID, any other alias, or runtime command. Use this for shortcuts such as `lc`, `omo`, or `omx`.

`display_name`: label shown in `hm harness list` and launch output.

`target_runtime`: existing runtime display name, for example `Codex CLI`, `Claude Code`, or `OpenCode`.

`detect_binaries`: executable names used for install status.

`launch_binary`: optional executable name to run instead of the target runtime binary.

`launch_args`: optional fixed args prepended before user args.

`package.self_update`: optional self-update stance for the package or wrapper.
Use `managed-by-hm` when installs and updates should go through `hm harness
install/update`, `suppressed-by-env` when launch-time upstream self-updaters are
disabled through `isolation.static_envs`, and `not-applicable` for manual
entries or packages with no self-update behavior. Bundled package-backed
harnesses must declare this field.

`package.bin_subdir`: optional relative bin directory under the isolated harness home. `npm-isolated` defaults this to `.npm/bin`; `python-tool` defaults it to `.local/bin`; `custom` may declare its own path. `hm harness list` and `hm use` use this field instead of package-kind-specific path guesses.

`package.kind = "custom"`: argv-only lifecycle commands for package systems hm does not know about. `install` is required; `update` and `uninstall` are optional. The first argv element must be an executable name, not a path or shell snippet, and args are passed directly without shell expansion.

After a successful install, hm records the resolved package manager under the harness isolation state and prefers it during update/remove. This keeps fallback chains such as `uv -> pipx -> pip -> pip3` and `bunx -> npx` stable across lifecycle commands.

`isolation.subdir`: optional runtime directory name. Defaults to `id`.

`isolation.home_subdirs`: directories created under the isolated home.

`isolation.static_envs`: static env values with `{home}`, `{state}`, `{tmp}`, `{runtime_home}`, `{runtime_state}`, and `{runtime_logs}` substitution. Prefer `{home}` for wrapper config roots; `{runtime_home}` is a compatibility token for advanced custom manifests.

`isolation.seed_files`: files created inside the isolation tree before launch or package-manager work. Wrapper config seeds should normally live under `{home}` so wrapper config stays isolated.

hm owns native runtime session sharing through runtime manifests, not harness manifests. Bundled wrappers keep config, plugins, cache, logs, and auth isolated, while manifest-declared session/transcript artifacts are linked from the user's native runtime home into the wrapper's isolated `{home}`.

## Isolation Tokens

Use these tokens instead of absolute host paths:

```text
{home}   isolated home directory
{state}  per-harness state directory
{tmp}    per-harness temp directory
{runtime_home}   target runtime home directory
{runtime_state}  target runtime state directory
{runtime_logs}   target runtime log directory
```

Seed file paths must start with `{home}/`, `{runtime_home}/`, `{state}/`, or `{tmp}/`. Prefer `{home}/` for wrapper config seeds so custom wrappers do not accidentally share native runtime config.

## Security Rules

Manifests are data, not code.

- No shell fragments.
- No absolute or relative-path launch binaries.
- No symlinked manifest files or plugin directories escaping discovery roots.
- No secret static env keys such as `*_TOKEN`, `*_SECRET`, or `*_API_KEY`.
- No static env override for `HOME`, `PATH`, `SHELL`, `PWD`, `_`, or `SSH_AUTH_SOCK`.
- No `LD_` or `DYLD_` static env keys.
- No package URLs, git specs, path package names, or option-looking package names.
- No seed files outside `{home}`, `{runtime_home}`, `{state}`, or `{tmp}`.

Each side-effecting command takes a per-harness runtime lock under `$XDG_DATA_HOME/hm/runtimes/.locks/`, so one harness cannot interleave launch/install/update/remove setup with itself.

## Bundled Harnesses

Bundled harnesses live in `harnesses/builtin/*.toml`. To add one to the repository, add a TOML file there. `build.rs` scans the directory and generates the builtin manifest index, so core Rust does not list individual harness IDs or package names.
