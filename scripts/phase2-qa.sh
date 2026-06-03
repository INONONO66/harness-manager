#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HM="$ROOT/target/release/hm"
EVIDENCE=$(mktemp -d -t hm-phase2-qa.XXXXXX)
export XDG_DATA_HOME="$EVIDENCE/xdg"
export PATH="$ROOT/target/release:$PATH"
HM_DATA="$XDG_DATA_HOME/hm"

cd "$ROOT"
echo "=== Building (release) ==="
cargo build --release 2>&1 | tail -3

pass() { printf "  \033[32m✓\033[0m %s\n" "$1"; }
fail() { printf "  \033[31m✗\033[0m %s\n" "$1"; echo "  evidence dir: $EVIDENCE"; exit 1; }
has() { grep -qE "$1" "$2" || fail "expected /$1/ in $2"; pass "matched: $1"; }
nohas() { grep -qE "$1" "$2" && fail "unexpected /$1/ in $2"; pass "absent: $1"; }
mode() { stat -f %Lp "$1"; }

echo ""
echo "=== S1: hm secret store ==="
printf '%s' 'sk-ant-test-phase2' | "$HM" secret set claude-api-key >"$EVIDENCE/s1.set.out" 2>"$EVIDENCE/s1.set.err"
test "$("$HM" secret get claude-api-key)" = "sk-ant-test-phase2" || fail "secret get mismatch"
pass "secret get returns exact value"
"$HM" secret list >"$EVIDENCE/s1.list"
has '^claude-api-key$' "$EVIDENCE/s1.list"
test "$(mode "$HM_DATA/secrets")" = "700" || fail "secrets dir mode is not 700"
test "$(mode "$HM_DATA/secrets/claude-api-key")" = "600" || fail "secret file mode is not 600"
pass "secret permissions are 700/600"

echo ""
echo "=== S2: Claude default apiKeyHelper isolation ==="
"$HM" use claude --print-env >"$EVIDENCE/s2.env" 2>"$EVIDENCE/s2.err"
has "^HOME=${HM_DATA}/runtimes/claude/home$" "$EVIDENCE/s2.env"
has "^CLAUDE_CONFIG_DIR=${HM_DATA}/runtimes/claude/home/\.claude$" "$EVIDENCE/s2.env"
has "^CLAUDE_CODE_TMPDIR=${HM_DATA}/runtimes/claude/tmp$" "$EVIDENCE/s2.env"
has "^CLAUDE_CODE_DEBUG_LOGS_DIR=${HM_DATA}/runtimes/claude/state/logs$" "$EVIDENCE/s2.env"
has '^DISABLE_LOGIN_COMMAND=1$' "$EVIDENCE/s2.env"
has '^DISABLE_UPDATES=1$' "$EVIDENCE/s2.env"
SETTINGS="$HM_DATA/runtimes/claude/home/.claude/settings.json"
HELPER="$HM_DATA/runtimes/claude/state/apikey.sh"
test -f "$SETTINGS" || fail "settings.json not seeded"
test -f "$HELPER" || fail "apikey.sh not seeded"
grep -q "apiKeyHelper" "$SETTINGS" || fail "settings.json missing apiKeyHelper"
test "$(mode "$HELPER")" = "700" || fail "apikey.sh mode is not 700"
test "$("$HELPER")" = "sk-ant-test-phase2" || fail "apikey helper did not return stored secret"
pass "Claude helper seeded and executable"

echo ""
echo "=== S3: Claude allow-keychain escape hatch ==="
"$HM" use claude --allow-keychain --print-env >"$EVIDENCE/s3.env" 2>"$EVIDENCE/s3.err"
has "^HOME=${HM_DATA}/runtimes/claude-keychain/home$" "$EVIDENCE/s3.env"
has "^CLAUDE_CONFIG_DIR=${HM_DATA}/runtimes/claude-keychain/home/\.claude$" "$EVIDENCE/s3.env"
nohas '^DISABLE_LOGIN_COMMAND=1$' "$EVIDENCE/s3.env"
test ! -f "$HM_DATA/runtimes/claude-keychain/home/.claude/settings.json" || fail "allow-keychain seeded settings.json"
test ! -f "$HM_DATA/runtimes/claude-keychain/state/apikey.sh" || fail "allow-keychain seeded apikey.sh"
grep -q 'Claude --allow-keychain mode permits OAuth' "$EVIDENCE/s3.err" || fail "allow-keychain warning missing"
pass "allow-keychain uses separate unseeded tree"

echo ""
echo "=== S4: allow-keychain rejected for non-Claude ==="
if "$HM" use codex --allow-keychain --print-env >"$EVIDENCE/s4.out" 2>"$EVIDENCE/s4.err"; then
  fail "codex --allow-keychain unexpectedly succeeded"
fi
grep -q -- '--allow-keychain is only supported for Claude Code' "$EVIDENCE/s4.err" || fail "non-Claude allow-keychain error missing"
pass "non-Claude allow-keychain rejected"

echo ""
echo "=== S5: secret rm ==="
"$HM" secret rm claude-api-key >/dev/null 2>&1
if "$HM" secret get claude-api-key >"$EVIDENCE/s5.get" 2>"$EVIDENCE/s5.err"; then
  fail "removed secret still readable"
fi
pass "secret rm removes stored key"

echo ""
printf "\033[32m═══════════════════════════════════════\033[0m\n"
printf "\033[32m  ALL PHASE 2 QA PASSED\033[0m\n"
printf "\033[32m═══════════════════════════════════════\033[0m\n"
echo "Evidence: $EVIDENCE"
