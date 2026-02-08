#!/usr/bin/env bash
# pawl Agent E2E tests — real AI agent (Claude CLI haiku) interacting with pawl.
# Prerequisites: pawl (cargo install --path .), jq, tmux, ccc/claude (Claude CLI)
# Usage: bash tests/e2e-agent.sh
# Cost: ~$0.05 per full run (haiku)

set -euo pipefail

# ── Prerequisite check ──
for cmd in tmux jq; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "SKIP: $cmd not found, agent tests require $cmd"
    exit 0
  fi
done

# Resolve Claude CLI to absolute path (aliases don't work in non-interactive scripts)
CCC="$(type -P ccc 2>/dev/null || type -P claude 2>/dev/null || true)"
if [ -z "$CCC" ]; then
  echo "SKIP: Claude CLI not found (tried ccc, claude)"
  exit 0
fi

# ── Resolve /tmp for macOS (/tmp → /private/tmp) ──
E2E_TMP="$(cd /tmp && pwd -P)"

# ── Session prefix for isolation ──
SESSION_PREFIX="pawl-e2e-agent"

# ── Results directory ──
RESULTS_DIR="${E2E_TMP}/pawl-e2e-agent-results"

# ── Portable timeout (macOS lacks timeout/gtimeout) ──
run_with_timeout() {
  local max_time="$1"; shift
  "$@" &
  local pid=$!
  ( sleep "$max_time" && kill "$pid" 2>/dev/null ) &
  local watchdog=$!
  wait "$pid" 2>/dev/null; local rc=$?
  kill "$watchdog" 2>/dev/null; wait "$watchdog" 2>/dev/null || true
  return $rc
}

# ── Helpers ──

assert_json() {
  local json="$1" expr="$2" expected="$3"
  local actual
  actual=$(echo "$json" | jq -r "$expr" 2>/dev/null) || { echo "jq parse error"; return 1; }
  if [ "$actual" != "$expected" ]; then
    echo "jq '$expr': expected '$expected', got '$actual'"
    return 1
  fi
}

assert_contains() {
  local haystack="$1" needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    echo "expected to contain '$needle'"
    return 1
  fi
}

setup_agent_project() {
  local name="$1" config="$2"
  local dir="${E2E_TMP}/pawl-e2e-agent-${name}"
  local session="${SESSION_PREFIX}-${name}"
  rm -rf "$dir"
  mkdir -p "$dir"
  cd "$dir"
  pawl init >/dev/null 2>&1
  echo "$config" | jq --arg s "$session" '. + {session: $s}' > .pawl/config.jsonc
}

create_task() {
  local name="$1"
  local body="${2:-}"
  if [ -n "$body" ]; then
    echo "$body" > ".pawl/tasks/${name}.md"
  else
    cat > ".pawl/tasks/${name}.md" <<EOF
---
name: ${name}
---

Task ${name}
EOF
  fi
}

wait_status() {
  local task="$1" status="$2" timeout="${3:-10}"
  local rc=0
  pawl wait "$task" --until "$status" -t "$timeout" >/dev/null 2>&1 || rc=$?
  if [ "$rc" != 0 ]; then
    echo "wait_status: wanted=$status, rc=$rc, actual=$(pawl status "$task" 2>/dev/null | jq -r '.status' 2>/dev/null || echo unknown)"
    return 1
  fi
}

# Call supervisor agent: reads supervise.md, executes pawl commands
# Uses --output-format stream-json --verbose for observability
# Uses --max-budget-usd to cap runaway costs
call_supervisor() {
  local project_dir="$1" prompt="$2" max_time="${3:-30}"
  local supervise_md
  supervise_md="$(cat "${project_dir}/.pawl/skills/pawl/references/supervise.md" 2>/dev/null || true)"

  run_with_timeout "$max_time" "$CCC" -p \
    --model haiku \
    --tools "Bash" \
    --permission-mode "bypassPermissions" \
    --system-prompt "$supervise_md" \
    --setting-sources "" --strict-mcp-config \
    --mcp-config '{"mcpServers":{}}' --disable-slash-commands \
    --output-format stream-json --verbose \
    --max-budget-usd 0.02 \
    "$prompt" >/dev/null 2>/dev/null || true
}

# Write a verifier wrapper script that calls Claude CLI with structured output
write_verifier_script() {
  local script_path="$1" prompt="$2"
  mkdir -p "$(dirname "$script_path")"
  cat > "$script_path" << VEOF
#!/usr/bin/env bash
SCHEMA='{"type":"object","properties":{"pass":{"type":"boolean"},"reason":{"type":"string"}},"required":["pass"]}'
result=\$("$CCC" -p --model haiku --tools "Bash" \\
  --permission-mode "bypassPermissions" \\
  --json-schema "\$SCHEMA" --output-format stream-json --verbose \\
  --setting-sources "" --strict-mcp-config \\
  --mcp-config '{"mcpServers":{}}' --disable-slash-commands \\
  --max-budget-usd 0.02 \\
  "$prompt" 2>/dev/null)
pass=\$(echo "\$result" | grep '"type":"result"' | jq -r '.structured_output.pass // false')
if [ "\$pass" = "true" ]; then
  exit 0
else
  reason=\$(echo "\$result" | grep '"type":"result"' | jq -r '.structured_output.reason // "rejected"')
  echo "\$reason" >&2
  exit 1
fi
VEOF
  chmod +x "$script_path"
}

# Cleanup all test tmux sessions and temp dirs
cleanup() {
  for sess in $(tmux list-sessions -F '#{session_name}' 2>/dev/null | grep "^${SESSION_PREFIX}" || true); do
    tmux kill-session -t "$sess" 2>/dev/null || true
  done
  rm -rf "${E2E_TMP}"/pawl-e2e-agent-*
}

trap cleanup EXIT

# ═══════════════════════════════════════════════════════
# Test Definitions
# ═══════════════════════════════════════════════════════

# ── Group 1: Supervisor routing (no viewport) ──

test_sup_gate() {
  setup_agent_project "gate" '{"workflow":[{"name":"approval"},{"name":"work","run":"true"}]}'
  local project_dir="$(pwd)"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "waiting" 10 || return 1

  call_supervisor "$project_dir" \
    "First run 'cd ${project_dir}', then run 'pawl status t1' and follow the routing hints. Execute the command in the prompt field."

  wait_status t1 "completed" 5 || return 1
  assert_json "$(pawl status t1 2>/dev/null)" ".status" "completed"
}

test_sup_verify_manual() {
  setup_agent_project "vman" '{"workflow":[{"name":"build","run":"true","verify":"manual"}]}'
  local project_dir="$(pwd)"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "waiting" 10 || return 1

  call_supervisor "$project_dir" \
    "First run 'cd ${project_dir}', then run 'pawl status t1'. The build passed and needs approval. Follow the prompt field."

  wait_status t1 "completed" 5 || return 1
  assert_json "$(pawl status t1 2>/dev/null)" ".status" "completed"
}

test_sup_on_fail_reset() {
  local counter="${E2E_TMP}/pawl-e2e-agent-ofr-count"
  rm -f "$counter"
  setup_agent_project "ofr" "{\"workflow\":[{\"name\":\"build\",\"run\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\",\"on_fail\":\"manual\"}]}"
  local project_dir="$(pwd)"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "waiting" 10 || return 1

  call_supervisor "$project_dir" \
    "First run 'cd ${project_dir}', then run 'pawl status t1'. The step failed. Execute the command in the suggest field to retry."

  wait_status t1 "completed" 5 || return 1
  assert_json "$(pawl status t1 2>/dev/null)" ".status" "completed"
  rm -f "$counter"
}

test_sup_multi_step() {
  setup_agent_project "multi" '{"workflow":[{"name":"gate1"},{"name":"work1","run":"true"},{"name":"gate2"},{"name":"work2","run":"true"}]}'
  local project_dir="$(pwd)"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "waiting" 10 || return 1

  call_supervisor "$project_dir" \
    "First run 'cd ${project_dir}', then loop: run 'pawl status t1', follow routing hints (prompt/suggest), check again. Repeat until status is 'completed'." 45

  wait_status t1 "completed" 5 || return 1
  assert_json "$(pawl status t1 2>/dev/null)" ".status" "completed"
}

# ── Group 2: Worker + Verifier (viewport) ──

test_worker_viewport() {
  local marker="${E2E_TMP}/pawl-e2e-agent-wk-marker"
  rm -f "$marker"

  setup_agent_project "wk" '{"workflow":[{"name":"work","run":"bash agents/worker.sh","in_viewport":true}]}'
  local project_dir="$(pwd)"
  mkdir -p agents
  cat > agents/worker.sh << WEOF
#!/usr/bin/env bash
"$CCC" -p --model haiku --tools "Bash" \\
  --permission-mode "bypassPermissions" \\
  --setting-sources "" --strict-mcp-config \\
  --mcp-config '{"mcpServers":{}}' --disable-slash-commands \\
  --output-format stream-json --verbose \\
  --max-budget-usd 0.01 \\
  "Run: echo AGENT_WORKED > $marker" >/dev/null 2>/dev/null
WEOF
  chmod +x agents/worker.sh

  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed,failed" 35 || return 1
  assert_json "$(pawl status t1 2>/dev/null)" ".status" "completed" || return 1
  [ -f "$marker" ] || { echo "marker file not created"; return 1; }
  assert_contains "$(cat "$marker")" "AGENT_WORKED"
}

test_verifier_pass() {
  local marker="${E2E_TMP}/pawl-e2e-agent-vp-marker"
  rm -f "$marker"
  echo "EXPECTED_CONTENT" > "$marker"
  setup_agent_project "vp" "{\"workflow\":[{\"name\":\"check\",\"run\":\"true\",\"verify\":\"bash agents/verifier.sh\"}]}"
  write_verifier_script "agents/verifier.sh" "Check if the file $marker contains EXPECTED_CONTENT. Read it with cat."
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  rm -f "$marker"
}

test_verifier_fail_retry() {
  local marker="${E2E_TMP}/pawl-e2e-agent-vfr-marker"
  local counter="${E2E_TMP}/pawl-e2e-agent-vfr-count"
  rm -f "$marker" "$counter"
  setup_agent_project "vfr" "{\"workflow\":[{\"name\":\"build\",\"run\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; if [ \$count -ge 2 ]; then echo CORRECT > $marker; else echo WRONG > $marker; fi\",\"verify\":\"bash agents/verifier.sh\",\"on_fail\":\"retry\",\"max_retries\":3}]}"
  write_verifier_script "agents/verifier.sh" "Check if file $marker contains the word CORRECT. Read it with cat. If CORRECT pass=true, if WRONG pass=false."
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  rm -f "$marker" "$counter"
}

# ── Group 3: Full feedback loop ──

test_feedback_loop() {
  local marker="${E2E_TMP}/pawl-e2e-agent-fb-marker"
  rm -f "$marker"

  setup_agent_project "fb" "{\"workflow\":[{\"name\":\"work\",\"run\":\"bash agents/worker-fb.sh\",\"in_viewport\":true,\"verify\":\"bash agents/verifier-fb.sh\",\"on_fail\":\"retry\",\"max_retries\":2}]}"

  # Worker: deterministic — checks PAWL_LAST_VERIFY_OUTPUT env var
  mkdir -p agents
  cat > agents/worker-fb.sh << WEOF
#!/usr/bin/env bash
if [ -n "\${PAWL_LAST_VERIFY_OUTPUT:-}" ]; then
  echo "CORRECTED" > $marker
else
  echo "INITIAL" > $marker
fi
WEOF
  chmod +x agents/worker-fb.sh

  # Verifier: only accepts CORRECTED
  write_verifier_script "agents/verifier-fb.sh" "Check if file $marker contains CORRECTED. Read with cat. If CORRECTED pass=true, otherwise pass=false."

  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed,failed" 60 || return 1
  assert_json "$(pawl status t1 2>/dev/null)" ".status" "completed" || return 1
  [ -f "$marker" ] || { echo "marker file not created"; return 1; }
  assert_contains "$(cat "$marker")" "CORRECTED"
  rm -f "$marker"
}

test_sup_multi_task() {
  setup_agent_project "mtask" '{"workflow":[{"name":"approval"},{"name":"work","run":"true"}]}'
  local project_dir="$(pwd)"
  create_task taskA
  create_task taskB
  pawl start taskA >/dev/null 2>&1
  pawl start taskB >/dev/null 2>&1
  wait_status taskA "waiting" 10 || return 1
  wait_status taskB "waiting" 10 || return 1

  call_supervisor "$project_dir" \
    "First run 'cd ${project_dir}', then run 'pawl done taskA' and 'pawl done taskB' to approve both."

  wait_status taskA "completed" 5 || return 1
  wait_status taskB "completed" 5 || return 1
  assert_json "$(pawl status taskA 2>/dev/null)" ".status" "completed" || return 1
  assert_json "$(pawl status taskB 2>/dev/null)" ".status" "completed"
}

# ═══════════════════════════════════════════════════════
# Test Registry
# ═══════════════════════════════════════════════════════

TESTS=(
  "test_sup_gate|supervisor: gate → done"
  "test_sup_verify_manual|supervisor: verify_manual → done"
  "test_sup_on_fail_reset|supervisor: on_fail_manual → reset --step"
  "test_sup_multi_step|supervisor: multi-step loop"
  "test_worker_viewport|worker: viewport agent creates file"
  "test_verifier_pass|verifier: agent verify → pass"
  "test_verifier_fail_retry|verifier: fail → retry → pass"
  "test_feedback_loop|feedback: worker → verifier reject → retry → corrected"
  "test_sup_multi_task|supervisor: multi-task list → done each"
)

# ═══════════════════════════════════════════════════════
# Parallel Runner
# ═══════════════════════════════════════════════════════

echo "── Agent E2E Tests (real Claude CLI haiku, ~\$0.05) ──"
echo ""
echo "  Running ${#TESTS[@]} tests in parallel..."
echo ""

rm -rf "$RESULTS_DIR" && mkdir -p "$RESULTS_DIR"

for entry in "${TESTS[@]}"; do
  func="${entry%%|*}"
  (
    set +e
    output=$("$func" 2>&1)
    rc=$?
    if [ $rc -eq 0 ]; then
      echo "PASS" > "${RESULTS_DIR}/${func}"
    else
      echo "${output}" > "${RESULTS_DIR}/${func}"
    fi
  ) &
done

wait

# ═══════════════════════════════════════════════════════
# Report
# ═══════════════════════════════════════════════════════

PASSED=0
FAILED=0
TOTAL=0
FAILURES=""

for entry in "${TESTS[@]}"; do
  func="${entry%%|*}"
  label="${entry#*|}"
  TOTAL=$((TOTAL + 1))

  result_file="${RESULTS_DIR}/${func}"
  if [ -f "$result_file" ] && [ "$(head -1 "$result_file")" = "PASS" ]; then
    PASSED=$((PASSED + 1))
    echo "  $label ... ok"
  else
    FAILED=$((FAILED + 1))
    local_msg=$(head -1 "$result_file" 2>/dev/null || echo "test did not complete")
    FAILURES="${FAILURES}\n  FAIL: ${label} — ${local_msg}"
    echo "  $label ... FAIL — ${local_msg}"
  fi
done

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Results: $PASSED/$TOTAL passed, $FAILED failed"
if [ $FAILED -gt 0 ]; then
  echo -e "$FAILURES"
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  exit 1
fi
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
