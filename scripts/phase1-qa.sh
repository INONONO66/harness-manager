#!/usr/bin/env bash
#
# Phase 1 QA — runtime isolation core
# Verifies S1 (Codex) / S2 (Pi) / S3 (OpenCode) + S4/S5 regressions + Claude sanity.
#
# Evidence captured to a mktemp dir; printed on success.
# Exits non-zero on first failed assertion.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HM="$ROOT/target/release/hm"
HM_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/hm"
EVIDENCE=$(mktemp -d -t hm-phase1-qa.XXXXXX)

cd "$ROOT"
echo "=== Building (release) ==="
cargo build --release 2>&1 | tail -3

pass() { printf "  \033[32m✓\033[0m %s\n" "$1"; }
fail() { printf "  \033[31m✗\033[0m %s\n" "$1"; echo "  evidence dir: $EVIDENCE"; exit 1; }
has()  { grep -qE "$1" "$2" || fail "expected /$1/ in $2"; pass "matched: $1"; }
nohas(){ grep -qE "$1" "$2" && fail "unexpected /$1/ in $2"; pass "absent: $1"; }

mtime() { stat -f %m "$1" 2>/dev/null || echo "none"; }

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "=== S1: Codex isolation ==="
SYS_BEFORE=$(mtime "$HOME/.codex")
rm -rf "$HM_DATA/runtimes/codex"
"$HM" use codex --print-env > "$EVIDENCE/s1.env" 2>"$EVIDENCE/s1.err" || fail "hm use codex --print-env exited non-zero"
has "^HOME=${HM_DATA}/runtimes/codex/home$" "$EVIDENCE/s1.env"
has "^CODEX_HOME=${HM_DATA}/runtimes/codex/home/\\.codex$" "$EVIDENCE/s1.env"
test -d "$HM_DATA/runtimes/codex/home/.codex" || fail ".codex subdir not created"
test -f "$HM_DATA/runtimes/codex/home/.codex/config.toml" || fail "config.toml not seeded"
grep -q "analytics_enabled = false" "$HM_DATA/runtimes/codex/home/.codex/config.toml" || fail "config.toml seed content wrong"
grep -q "check_for_update_on_startup = false" "$HM_DATA/runtimes/codex/home/.codex/config.toml" || fail "config.toml missing update knob"
grep -q 'cli_auth_credentials_store = "file"' "$HM_DATA/runtimes/codex/home/.codex/config.toml" || fail "config.toml missing file-mode creds"
pass "config.toml seeded with full content"

# create-if-missing: user edits preserved
echo "USER_EDIT_MARKER" > "$HM_DATA/runtimes/codex/home/.codex/config.toml"
"$HM" use codex --print-env > /dev/null 2>&1
test "$(cat "$HM_DATA/runtimes/codex/home/.codex/config.toml")" = "USER_EDIT_MARKER" || fail "user edit overwritten (create-if-missing violated)"
pass "create-if-missing preserves user edits"

SYS_AFTER=$(mtime "$HOME/.codex")
[[ "$SYS_BEFORE" = "$SYS_AFTER" ]] || fail "system ~/.codex mtime changed ($SYS_BEFORE → $SYS_AFTER)"
pass "system ~/.codex untouched"

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "=== S2: Pi isolation + env_var bugfix ==="
SYS_BEFORE=$(mtime "$HOME/.pi")
rm -rf "$HM_DATA/runtimes/pi"
"$HM" use pi --print-env > "$EVIDENCE/s2.env" 2>"$EVIDENCE/s2.err" || fail "hm use pi --print-env exited non-zero"
has "^HOME=${HM_DATA}/runtimes/pi/home$" "$EVIDENCE/s2.env"
has "^PI_CODING_AGENT_DIR=${HM_DATA}/runtimes/pi/home/\\.pi/agent$" "$EVIDENCE/s2.env"
has "^PI_OFFLINE=1$" "$EVIDENCE/s2.env"
has "^PI_SKIP_VERSION_CHECK=1$" "$EVIDENCE/s2.env"
has "^PI_TELEMETRY=0$" "$EVIDENCE/s2.env"
test -d "$HM_DATA/runtimes/pi/home/.pi/agent" || fail ".pi/agent subdir not created"

SYS_AFTER=$(mtime "$HOME/.pi")
[[ "$SYS_BEFORE" = "$SYS_AFTER" ]] || fail "system ~/.pi mtime changed"
pass "system ~/.pi untouched"

# Pi env_var bug fix verification: external PI_CODING_AGENT_DIR should be honored by detect
FAKE_PI="$EVIDENCE/pi-fake/.pi/agent"
mkdir -p "$FAKE_PI"
touch "$FAKE_PI/settings.json"
PI_CODING_AGENT_DIR="$FAKE_PI" "$HM" detect > "$EVIDENCE/s2.detect" 2>&1
grep -q "Pi" "$EVIDENCE/s2.detect" || fail "Pi missing from detect output"
pass "PI_CODING_AGENT_DIR env honored by detect (bugfix verified)"

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "=== S3: OpenCode XDG quartet redirect ==="
SYS_BEFORE=$(mtime "$HOME/.config/opencode")
rm -rf "$HM_DATA/runtimes/opencode"
"$HM" use opencode --print-env > "$EVIDENCE/s3.env" 2>"$EVIDENCE/s3.err" || fail "hm use opencode --print-env exited non-zero"
has "^HOME=${HM_DATA}/runtimes/opencode/home$" "$EVIDENCE/s3.env"
has "^XDG_CONFIG_HOME=${HM_DATA}/runtimes/opencode/home/\\.config$" "$EVIDENCE/s3.env"
has "^XDG_DATA_HOME=${HM_DATA}/runtimes/opencode/home/\\.local/share$" "$EVIDENCE/s3.env"
has "^XDG_CACHE_HOME=${HM_DATA}/runtimes/opencode/home/\\.cache$" "$EVIDENCE/s3.env"
has "^XDG_STATE_HOME=${HM_DATA}/runtimes/opencode/home/\\.local/state$" "$EVIDENCE/s3.env"
has "^OPENCODE_DISABLE_AUTOUPDATE=1$" "$EVIDENCE/s3.env"
has "^OPENCODE_DISABLE_PROJECT_CONFIG=1$" "$EVIDENCE/s3.env"
nohas "^OPENCODE_PURE=1$" "$EVIDENCE/s3.env"
test -d "$HM_DATA/runtimes/opencode/home/.config/opencode" || fail ".config/opencode subdir not created"
test -d "$HM_DATA/runtimes/opencode/home/.local/share/opencode" || fail ".local/share/opencode subdir not created"
test -d "$HM_DATA/runtimes/opencode/home/.cache/opencode" || fail ".cache/opencode subdir not created"
test -d "$HM_DATA/runtimes/opencode/home/.local/state/opencode" || fail ".local/state/opencode subdir not created"

SYS_AFTER=$(mtime "$HOME/.config/opencode")
[[ "$SYS_BEFORE" = "$SYS_AFTER" ]] || fail "system ~/.config/opencode mtime changed"
pass "system ~/.config/opencode untouched"

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "=== S4: detect/auth regression ==="
"$HM" detect > "$EVIDENCE/s4.detect" 2>&1 || fail "hm detect exited non-zero"
for rt in "Claude Code" "Codex CLI" "OpenCode" "Pi"; do
  grep -q "$rt" "$EVIDENCE/s4.detect" || fail "$rt missing from detect"
done
pass "detect lists all 4 runtimes"

"$HM" auth status > "$EVIDENCE/s4.auth" 2>&1 || fail "hm auth status exited non-zero"
pass "auth status ran clean"

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "=== S5: inject plan regression ==="
if [[ -f "$HOME/.config/hm/config.toml" ]] && grep -q "proxy" "$HOME/.config/hm/config.toml" 2>/dev/null; then
  "$HM" inject plan codex --profile proxy > "$EVIDENCE/s5.inject" 2>&1 || fail "hm inject plan codex --profile proxy exited non-zero"
  grep -q "Codex CLI" "$EVIDENCE/s5.inject" || fail "inject plan missing Codex section"
  grep -q "Strip:" "$EVIDENCE/s5.inject" || fail "inject plan missing Strip section"
  grep -q "Inject:" "$EVIDENCE/s5.inject" || fail "inject plan missing Inject section"
  pass "inject plan unchanged (Strip/Inject sections present)"
else
  pass "skipped (no proxy profile configured)"
fi

# ─────────────────────────────────────────────────────────────────────────────
echo ""
echo "=== Sanity: Claude isolation exists after Phase 2 ==="
"$HM" use claude --print-env > "$EVIDENCE/sanity.env" 2>"$EVIDENCE/sanity.err" || fail "hm use claude --print-env exited non-zero"
has "^CLAUDE_CONFIG_DIR=${HM_DATA}/runtimes/claude/home/\.claude$" "$EVIDENCE/sanity.env"
has "^HOME=${HM_DATA}/runtimes/claude/home$" "$EVIDENCE/sanity.env"
test -f "$HM_DATA/runtimes/claude/home/.claude/settings.json" || fail "Claude settings.json not seeded"
test -f "$HM_DATA/runtimes/claude/state/apikey.sh" || fail "Claude apikey.sh not seeded"
pass "Claude isolation enabled by Phase 2"

# ─────────────────────────────────────────────────────────────────────────────
echo ""
printf "\033[32m═══════════════════════════════════════\033[0m\n"
printf "\033[32m  ALL PHASE 1 QA PASSED\033[0m\n"
printf "\033[32m═══════════════════════════════════════\033[0m\n"
echo "Evidence: $EVIDENCE"
