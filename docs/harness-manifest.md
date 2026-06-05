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

Check it:

```bash
hm harness list
hm harness install demo
hm use demo -- --help
hm harness remove demo --purge
```

## Choose An Install Strategy

Use one structured `package.kind`. `hm` never runs shell snippets from manifests.

```toml
[package]
kind = "npm-global"
package = "my-package"
```

```toml
[package]
kind = "npx-installer"
package = "my-package"
args = ["setup"]
```

```toml
[package]
kind = "bunx-installer"
package = "my-package"
args = ["install"]
```

```toml
[package]
kind = "python-tool"
package = "my-package"
```

```toml
[package]
kind = "manual"
instructions = "Install the binary with your plugin manager."
```

## Field Reference

`schema_version`: must be `1`.

`id`: command identifier. Lowercase ASCII, 2-64 characters, with digits, `_`, and `-` allowed. It must not duplicate a bundled/user harness ID or shadow a runtime command.

`display_name`: label shown in `hm harness list` and launch output.

`target_runtime`: existing runtime display name, for example `Codex CLI`, `Claude Code`, or `OpenCode`.

`detect_binaries`: executable names used for install status.

`launch_binary`: optional executable name to run instead of the target runtime binary.

`launch_args`: optional fixed args prepended before user args.

`isolation.subdir`: optional runtime directory name. Defaults to `id`.

`isolation.spoof_home`: when true, `HOME` points at the harness isolation home.

`isolation.home_subdirs`: directories created under the isolated home.

`isolation.static_envs`: static env values with `{home}`, `{state}`, and `{tmp}` substitution.

`isolation.seed_files`: files created inside the isolation tree before launch or package-manager work.

## Isolation Tokens

Use these tokens instead of absolute host paths:

```text
{home}   isolated home directory
{state}  per-harness state directory
{tmp}    per-harness temp directory
```

Seed file paths must start with `{home}/`, `{state}/`, or `{tmp}/`.

## Security Rules

Manifests are data, not code.

- No shell fragments.
- No absolute or relative-path launch binaries.
- No symlinked manifest files or plugin directories escaping discovery roots.
- No secret static env keys such as `*_TOKEN`, `*_SECRET`, or `*_API_KEY`.
- No static env override for `HOME`, `PATH`, `SHELL`, `PWD`, `_`, or `SSH_AUTH_SOCK`.
- No `LD_` or `DYLD_` static env keys.
- No package URLs, git specs, path package names, or option-looking package names.
- No seed files outside `{home}`, `{state}`, or `{tmp}`.

Each side-effecting command takes a per-harness lock under `$XDG_DATA_HOME/hm/runtimes/.locks/`, so install/update/remove and launch setup do not interleave for the same harness.

## Bundled Harnesses

Bundled harnesses live in `harnesses/builtin/*.toml`. To add one to the repository, add a TOML file there. `build.rs` scans the directory and generates the builtin manifest index, so core Rust does not list individual harness IDs or package names.
