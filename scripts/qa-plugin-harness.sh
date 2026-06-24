#!/usr/bin/env bash
set -euo pipefail

mode="${1:-lifecycle}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo build >/dev/null
hm_bin="$repo_root/target/debug/hm"
qa_root="$(mktemp -d /tmp/hm-plugin-qa.XXXXXX)"
export XDG_CONFIG_HOME="$qa_root/config"
export XDG_DATA_HOME="$qa_root/data"
export HM_QA_LOG_DIR="$qa_root/logs"
export PATH="$qa_root/bin:$PATH"
export OPENAI_API_KEY="leak-openai"
export GITHUB_TOKEN="leak-github"
export NPM_TOKEN="leak-npm"
export SSH_AUTH_SOCK="$qa_root/agent.sock"
export CODEX_HOME="$qa_root/real-codex"

mkdir -p "$qa_root/bin" "$HM_QA_LOG_DIR" "$XDG_CONFIG_HOME/hm/harnesses.d"

cleanup() {
  rm -rf "$qa_root"
}
trap cleanup EXIT

write_fake_command() {
  local name="$1"
  local log_file="$HM_QA_LOG_DIR/$name.log"
  cat > "$qa_root/bin/$name" <<FAKE
#!/usr/bin/env bash
set -euo pipefail
log_file="$log_file"
{
  printf 'ARGV=%s\n' "\$*"
  env | sort
  printf '%s\n' '---'
} >> "\$log_file"
FAKE
  chmod +x "$qa_root/bin/$name"
}

write_manifest() {
  local id="$1"
  local binary="$2"
  local package_kind="${3:-npm-global}"
  local package_block
  case "$package_kind" in
    npm-global)
      package_block=$'[package]\nkind = "npm-global"\npackage = "demo-package"'
      ;;
    manual)
      package_block=$'[package]\nkind = "manual"\ninstructions = "manual"'
      ;;
    *)
      echo "unsupported package kind: $package_kind" >&2
      exit 2
      ;;
  esac
  cat > "$XDG_CONFIG_HOME/hm/harnesses.d/$id.toml" <<MANIFEST
schema_version = 1
id = "$id"
display_name = "Demo Harness $id"
target_runtime = "Codex CLI"
detect_binaries = ["$binary"]
launch_binary = "$binary"
launch_args = ["--fixed"]

$package_block

[isolation]
subdir = "$id"
home_subdirs = []
static_envs = { CODEX_HOME = "{runtime_home}/.codex", DEMO_STATE = "{state}/$id", DEMO_LOGS = "{runtime_logs}" }

[[isolation.seed_files]]
path = "{runtime_home}/.codex/config.toml"
content = "analytics_enabled = false\\n"
overwrite = false
MANIFEST
}

write_config() {
  cat > "$XDG_CONFIG_HOME/hm/config.toml" <<'CONFIG'
default_profile = "proxy"

[profiles.proxy]
description = "QA proxy"

[profiles.proxy.llm]
endpoint = "https://proxy.example.test/v1"
bearer = "qa-token"
CONFIG
}

assert_no_hostile_values() {
  local file="$1"
  ! grep -E 'leak-openai|leak-github|leak-npm|real-codex|SSH_AUTH_SOCK=' "$file" >/dev/null
}

run_lifecycle() {
  write_config
  for bin in npm npx bunx uv pipx pip pip3 demo-agent; do
    write_fake_command "$bin"
  done
  write_manifest demo demo-agent npm-global

  "$hm_bin" harness list > "$qa_root/list.out" 2>&1
  grep 'demo' "$qa_root/list.out" >/dev/null
  echo 'PASS list'

  "$hm_bin" harness install demo > "$qa_root/install.out" 2>&1
  grep 'ARGV=install -g demo-package' "$HM_QA_LOG_DIR/npm.log" >/dev/null
  assert_no_hostile_values "$HM_QA_LOG_DIR/npm.log"
  echo 'PASS install'

  "$hm_bin" harness update demo > "$qa_root/update.out" 2>&1
  grep 'ARGV=update -g demo-package' "$HM_QA_LOG_DIR/npm.log" >/dev/null
  assert_no_hostile_values "$HM_QA_LOG_DIR/npm.log"
  echo 'PASS update'

  "$hm_bin" use demo --print-env > "$qa_root/use-env.out" 2>&1
  grep "HOME=$XDG_DATA_HOME/hm/runtimes/demo/home" "$qa_root/use-env.out" >/dev/null
  grep "CODEX_HOME=$XDG_DATA_HOME/hm/runtimes/codex/home/.codex" "$qa_root/use-env.out" >/dev/null
  grep "DEMO_LOGS=$XDG_DATA_HOME/hm/runtimes/codex/state/logs" "$qa_root/use-env.out" >/dev/null
  test -d "$XDG_DATA_HOME/hm/runtimes/codex/home"
  test -d "$XDG_DATA_HOME/hm/runtimes/codex/state/logs"
  assert_no_hostile_values "$qa_root/use-env.out"
  echo 'PASS use'

  "$hm_bin" demo -- --probe > "$qa_root/launch.out" 2>&1
  grep 'ARGV=--fixed -- --probe' "$HM_QA_LOG_DIR/demo-agent.log" >/dev/null
  assert_no_hostile_values "$HM_QA_LOG_DIR/demo-agent.log"
  echo 'PASS launch'

  "$hm_bin" inject plan demo --profile proxy > "$qa_root/inject.out" 2>&1
  grep 'Demo Harness demo (Codex CLI)' "$qa_root/inject.out" >/dev/null
  grep 'https://proxy.example.test/v1' "$qa_root/inject.out" >/dev/null
  echo 'PASS inject'

  "$hm_bin" harness remove demo --purge > "$qa_root/remove.out" 2>&1
  grep 'ARGV=uninstall -g demo-package' "$HM_QA_LOG_DIR/npm.log" >/dev/null
  test ! -e "$XDG_DATA_HOME/hm/runtimes/demo"
  test -e "$XDG_DATA_HOME/hm/runtimes/codex"
  echo 'PASS remove'
}

run_invalid() {
  write_config
  for bin in npm npx bunx uv pipx pip pip3 demo-agent; do
    write_fake_command "$bin"
  done
  write_manifest demo demo-agent npm-global
  cp tests/fixtures/harnesses/bad-env.toml "$XDG_CONFIG_HOME/hm/harnesses.d/bad.toml"

  for command in \
    "harness list" \
    "harness install demo" \
    "harness update demo" \
    "use demo --print-env" \
    "inject plan demo --profile proxy" \
    "harness remove demo --purge"
  do
    set +e
    "$hm_bin" $command > "$qa_root/invalid.out" 2>&1
    status=$?
    set -e
    test "$status" -ne 0
    grep 'bad.toml' "$qa_root/invalid.out" >/dev/null
  done
  test -z "$(find "$HM_QA_LOG_DIR" -type f -print -quit)"
  test ! -e "$XDG_DATA_HOME/hm/runtimes/demo"
  echo 'PASS invalid manifest blocked before side effects'
}

run_concurrent() {
  write_fake_command demo-agent-a
  write_fake_command demo-agent-b
  write_manifest demo-a demo-agent-a manual
  write_manifest demo-b demo-agent-b manual

  "$hm_bin" use demo-a --print-env > "$qa_root/demo-a.env" 2>&1 &
  pid_a=$!
  "$hm_bin" use demo-b --print-env > "$qa_root/demo-b.env" 2>&1 &
  pid_b=$!
  wait "$pid_a"
  wait "$pid_b"

  grep "CODEX_HOME=$XDG_DATA_HOME/hm/runtimes/codex/home/.codex" "$qa_root/demo-a.env" >/dev/null
  grep "DEMO_STATE=$XDG_DATA_HOME/hm/runtimes/demo-a/state/demo-a" "$qa_root/demo-a.env" >/dev/null
  grep "DEMO_LOGS=$XDG_DATA_HOME/hm/runtimes/codex/state/logs" "$qa_root/demo-a.env" >/dev/null
  grep "CODEX_HOME=$XDG_DATA_HOME/hm/runtimes/codex/home/.codex" "$qa_root/demo-b.env" >/dev/null
  grep "DEMO_STATE=$XDG_DATA_HOME/hm/runtimes/demo-b/state/demo-b" "$qa_root/demo-b.env" >/dev/null
  grep "DEMO_LOGS=$XDG_DATA_HOME/hm/runtimes/codex/state/logs" "$qa_root/demo-b.env" >/dev/null
  ! grep 'demo-b' "$qa_root/demo-a.env" >/dev/null
  ! grep 'demo-a' "$qa_root/demo-b.env" >/dev/null
  echo 'PASS concurrent demo-a'
  echo 'PASS concurrent demo-b'
  echo 'PASS no cross contamination'
}

case "$mode" in
  lifecycle)
    if [[ "${BAD_MANIFEST:-0}" == "1" ]]; then
      run_invalid
    else
      run_lifecycle
    fi
    ;;
  --concurrent)
    run_concurrent
    ;;
  *)
    echo "unknown mode: $mode" >&2
    exit 2
    ;;
esac

cleanup
trap - EXIT
echo 'PASS cleanup'
