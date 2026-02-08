#!/usr/bin/env bash
# pawl Viewport E2E tests — parallel execution.
# Prerequisites: pawl (cargo install --path .), jq, tmux
# Usage: bash tests/e2e-viewport.sh

set -euo pipefail

# ── Prerequisite check ──
if ! command -v tmux &>/dev/null; then
  echo "SKIP: tmux not found, viewport tests require tmux"
  exit 0
fi

# ── Resolve /tmp for macOS (/tmp → /private/tmp) ──
E2E_TMP="$(cd /tmp && pwd -P)"

# ── Session prefix for isolation ──
SESSION_PREFIX="pawl-e2e-vp"

# ── Results directory ──
RESULTS_DIR="${E2E_TMP}/pawl-e2e-vp-results"

# ── Helpers (used inside subshells — no global state) ──

assert_exit() {
  local expected="$1" actual="$2"
  if [ "$expected" != "$actual" ]; then
    echo "expected exit $expected, got $actual"
    return 1
  fi
}

assert_json() {
  local json="$1" expr="$2" expected="$3"
  local actual
  actual=$(echo "$json" | jq -r "$expr" 2>/dev/null) || { echo "jq parse error"; return 1; }
  if [ "$actual" != "$expected" ]; then
    echo "jq '$expr': expected '$expected', got '$actual'"
    return 1
  fi
}

assert_json_num() {
  local json="$1" expr="$2" expected="$3"
  local actual
  actual=$(echo "$json" | jq "$expr" 2>/dev/null) || { echo "jq parse error"; return 1; }
  if [ "$actual" != "$expected" ]; then
    echo "jq '$expr': expected $expected, got $actual"
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

# Setup a viewport project in /tmp with unique tmux session
setup_vp_project() {
  local name="$1" config="$2"
  local dir="${E2E_TMP}/pawl-e2e-vp-${name}"
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
  local task="$1" status="$2" timeout="${3:-5}"
  local rc=0
  pawl wait "$task" --until "$status" -t "$timeout" >/dev/null 2>&1 || rc=$?
  if [ "$rc" != 0 ]; then
    echo "wait_status: wanted=$status, rc=$rc"
    pawl status "$task" 2>/dev/null || true
    return 1
  fi
}

# Cleanup all test tmux sessions and temp dirs
cleanup() {
  for sess in $(tmux list-sessions -F '#{session_name}' 2>/dev/null | grep "^${SESSION_PREFIX}" || true); do
    tmux kill-session -t "$sess" 2>/dev/null || true
  done
  rm -rf "${E2E_TMP}"/pawl-e2e-vp-*
}

trap cleanup EXIT

# ═══════════════════════════════════════════════════════
# Test Definitions
# Each returns 0 (pass) or 1 (fail, error message on stdout)
# ═══════════════════════════════════════════════════════

test_vp_auto_complete() {
  setup_vp_project "auto-ok" '{"workflow":[{"name":"fast","run":"true","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 5 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_auto_fail() {
  setup_vp_project "auto-fail" '{"workflow":[{"name":"bad","run":"false","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "failed" 5 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "failed"
}

test_vp_multi_step() {
  setup_vp_project "multi" '{"workflow":[{"name":"prep","run":"true"},{"name":"work","run":"true","in_viewport":true},{"name":"fin","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 10 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  assert_json_num "$out" ".current_step" "3"
}

test_vp_done_completes() {
  setup_vp_project "done-ok" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_done_with_verify_manual() {
  setup_vp_project "done-vman" '{"workflow":[{"name":"work","run":"true","in_viewport":true,"verify":"manual"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "waiting" 5 || return 1
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_done_with_verify_cmd() {
  setup_vp_project "done-vcmd" '{"workflow":[{"name":"work","run":"true","in_viewport":true,"verify":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 5 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_retry_auto() {
  local counter="${E2E_TMP}/pawl-e2e-vp-retry-count"
  rm -f "$counter"
  setup_vp_project "retry-ok" "{\"workflow\":[{\"name\":\"flaky\",\"run\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\",\"in_viewport\":true,\"on_fail\":\"retry\",\"max_retries\":3}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 10 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  rm -f "$counter"
}

test_vp_retry_exhaustion() {
  setup_vp_project "retry-ex" '{"workflow":[{"name":"bad","run":"false","in_viewport":true,"on_fail":"retry","max_retries":1}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "failed" 10 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "failed"
}

test_vp_on_fail_manual() {
  setup_vp_project "ofm" '{"workflow":[{"name":"bad","run":"false","in_viewport":true,"on_fail":"manual"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "waiting" 5 || return 1
  local status_out
  status_out=$(pawl status t1 2>/dev/null)
  assert_json "$status_out" ".message" "on_fail_manual" || return 1
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_loss_detected() {
  local session="${SESSION_PREFIX}-loss"
  setup_vp_project "loss" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  tmux kill-window -t "${session}:t1" 2>/dev/null || true
  sleep 0.3
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "failed"
}

test_vp_loss_recovery() {
  local session="${SESSION_PREFIX}-loss-rec"
  setup_vp_project "loss-rec" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  tmux kill-window -t "${session}:t1" 2>/dev/null || true
  sleep 0.3
  pawl status t1 >/dev/null 2>/dev/null
  pawl reset --step t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_capture_content() {
  setup_vp_project "cap-content" '{"workflow":[{"name":"echo","run":"echo PAWL_MARKER_12345; sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.8
  local out
  out=$(pawl capture t1 2>/dev/null)
  assert_json "$out" ".viewport_exists" "true" || return 1
  local content
  content=$(echo "$out" | jq -r '.content')
  assert_contains "$content" "PAWL_MARKER_12345" || return 1
  pawl done t1 >/dev/null 2>&1 || true
}

test_vp_capture_no_viewport() {
  setup_vp_project "cap-none" '{"workflow":[{"name":"work","run":"true","in_viewport":true}]}'
  create_task t1
  local out
  out=$(pawl capture t1 2>/dev/null)
  assert_json "$out" ".viewport_exists" "false"
}

test_vp_capture_active() {
  setup_vp_project "cap-active" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  local out
  out=$(pawl capture t1 2>/dev/null)
  assert_json "$out" ".viewport_exists" "true" || return 1
  assert_json "$out" ".process_active" "true" || return 1
  pawl done t1 >/dev/null 2>&1
  sleep 0.2
  out=$(pawl capture t1 2>/dev/null)
  assert_json "$out" ".viewport_exists" "false"
}

test_vp_enter() {
  setup_vp_project "enter-ok" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  local rc=0
  pawl enter t1 >/dev/null 2>&1 || rc=$?
  assert_exit 0 "$rc" || return 1
  pawl done t1 >/dev/null 2>&1 || true
}

test_vp_enter_no_viewport() {
  setup_vp_project "enter-none" '{"workflow":[{"name":"work","run":"true","in_viewport":true}]}'
  create_task t1
  local rc=0
  pawl enter t1 >/dev/null 2>&1 || rc=$?
  assert_exit 4 "$rc"
}

test_vp_stop() {
  setup_vp_project "stop" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  local out
  out=$(pawl stop t1 2>/dev/null)
  assert_json "$out" ".status" "stopped"
}

test_vp_env_vars() {
  local marker="${E2E_TMP}/pawl-e2e-vp-envmarker"
  rm -f "$marker"
  setup_vp_project "envvars" "{\"workflow\":[{\"name\":\"check\",\"run\":\"echo \$PAWL_TASK:\$PAWL_STEP > $marker\",\"in_viewport\":true}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 5 || return 1
  [ -f "$marker" ] || { echo "marker file not created"; return 1; }
  local content
  content=$(cat "$marker")
  assert_contains "$content" "t1:check" || return 1
  rm -f "$marker"
}

test_vp_consecutive() {
  local marker="${E2E_TMP}/pawl-e2e-vp-consec-marker"
  rm -f "$marker"
  setup_vp_project "consec" "{\"workflow\":[{\"name\":\"step1\",\"run\":\"echo A >> $marker\",\"in_viewport\":true},{\"name\":\"step2\",\"run\":\"echo B >> $marker\",\"in_viewport\":true}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 10 || return 1
  [ -f "$marker" ] || { echo "marker file not created"; return 1; }
  local content
  content=$(cat "$marker")
  assert_contains "$content" "A" || return 1
  assert_contains "$content" "B" || return 1
  rm -f "$marker"
}

test_vp_consecutive_three() {
  local marker="${E2E_TMP}/pawl-e2e-vp-consec3-marker"
  rm -f "$marker"
  setup_vp_project "consec3" "{\"workflow\":[{\"name\":\"a\",\"run\":\"echo 1 >> $marker\",\"in_viewport\":true},{\"name\":\"b\",\"run\":\"echo 2 >> $marker\",\"in_viewport\":true},{\"name\":\"c\",\"run\":\"echo 3 >> $marker\",\"in_viewport\":true}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 10 || return 1
  [ -f "$marker" ] || { echo "marker file not created"; return 1; }
  local lines
  lines=$(wc -l < "$marker" | tr -d ' ')
  [ "$lines" = "3" ] || { echo "expected 3 lines, got $lines"; return 1; }
  rm -f "$marker"
}

test_vp_done_message() {
  setup_vp_project "done-msg" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  pawl done -m "agent approved this" t1 >/dev/null 2>&1
  local log_out
  log_out=$(pawl log --all t1 2>/dev/null)
  assert_contains "$log_out" "agent approved this"
}

test_vp_done_message_gate() {
  setup_vp_project "done-msg-gate" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  pawl done -m "approved by agent" t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "completed"
}

test_vp_skip() {
  setup_vp_project "skip-vp" '{"workflow":[{"name":"dangerous","run":"sleep 60","in_viewport":true},{"name":"fin","run":"true"}]}'
  create_task t1 "---
name: t1
skip:
  - dangerous
---
Task t1"
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  local session="${SESSION_PREFIX}-skip-vp"
  local has_window
  has_window=$(tmux list-windows -t "$session" -F '#{window_name}' 2>/dev/null | grep -c '^t1$' || true)
  [ "$has_window" = "0" ] || { echo "viewport window should not exist"; return 1; }
}

test_vp_hook_viewport_launched() {
  local marker="${E2E_TMP}/pawl-e2e-vp-hook-launched"
  rm -f "$marker"
  setup_vp_project "hook-launch" "{\"workflow\":[{\"name\":\"work\",\"run\":\"sleep 60\",\"in_viewport\":true}],\"on\":{\"viewport_launched\":\"touch $marker\"}}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.5
  [ -f "$marker" ] || { echo "hook marker not found"; return 1; }
  pawl done t1 >/dev/null 2>&1 || true
  rm -f "$marker"
}

test_vp_hook_viewport_lost() {
  local marker="${E2E_TMP}/pawl-e2e-vp-hook-lost-marker"
  local session="${SESSION_PREFIX}-hook-lost"
  rm -f "$marker"
  # Use unique sleep duration to avoid killing other tests' processes
  setup_vp_project "hook-lost" "{\"workflow\":[{\"name\":\"work\",\"run\":\"sleep 12345\",\"in_viewport\":true}],\"on\":{\"viewport_lost\":\"touch $marker\"}}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.5
  # Kill _run via tmux pane PID (session-specific, safe for parallel)
  local pane_pid
  pane_pid=$(tmux display-message -t "${session}:t1" -p '#{pane_pid}' 2>/dev/null || true)
  if [ -n "$pane_pid" ]; then
    pkill -9 -P "$pane_pid" 2>/dev/null || true
    kill -9 "$pane_pid" 2>/dev/null || true
  fi
  sleep 0.5
  tmux kill-window -t "${session}:t1" 2>/dev/null || true
  sleep 0.3
  pawl status t1 >/dev/null 2>/dev/null || true
  sleep 1.5
  [ -f "$marker" ] || { echo "hook marker not found"; return 1; }
  rm -f "$marker"
}

test_vp_verify_fail_retry() {
  local counter="${E2E_TMP}/pawl-e2e-vp-vfr-count"
  rm -f "$counter"
  setup_vp_project "vfr" "{\"workflow\":[{\"name\":\"build\",\"run\":\"true\",\"in_viewport\":true,\"verify\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\",\"on_fail\":\"retry\",\"max_retries\":3}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 10 || return 1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  rm -f "$counter"
}

test_vp_task_index() {
  setup_vp_project "idx" '{"workflow":[{"name":"work","run":"true","in_viewport":true}]}'
  create_task alpha
  create_task beta
  pawl start 1 >/dev/null 2>&1
  wait_status alpha "completed" 5 || return 1
  local out
  out=$(pawl status 1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return 1
  assert_json "$out" ".name" "alpha"
}

test_vp_task_index_invalid() {
  setup_vp_project "idx-inv" '{"workflow":[{"name":"work","run":"true"}]}'
  create_task only
  local rc=0
  pawl start 99 >/dev/null 2>&1 || rc=$?
  assert_exit 4 "$rc" || return 1
  rc=0
  pawl start 0 >/dev/null 2>&1 || rc=$?
  assert_exit 4 "$rc"
}

test_vp_full_reset_running() {
  setup_vp_project "fullrst" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  sleep 0.3
  local out
  out=$(pawl reset t1 2>/dev/null)
  assert_json "$out" ".status" "pending" || return 1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || return 1
  pawl done t1 >/dev/null 2>&1 || true
}

test_vp_multi_task() {
  setup_vp_project "mtask" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task taskA
  create_task taskB
  pawl start taskA >/dev/null 2>&1
  pawl start taskB >/dev/null 2>&1
  wait_status taskA "running" 5 || return 1
  wait_status taskB "running" 5 || return 1
  local session="${SESSION_PREFIX}-mtask"
  local winA winB
  winA=$(tmux list-windows -t "$session" -F '#{window_name}' 2>/dev/null | grep -c '^taskA$' || true)
  winB=$(tmux list-windows -t "$session" -F '#{window_name}' 2>/dev/null | grep -c '^taskB$' || true)
  [ "$winA" = "1" ] || { echo "taskA window missing"; return 1; }
  [ "$winB" = "1" ] || { echo "taskB window missing"; return 1; }
  pawl done taskA >/dev/null 2>&1
  local outA outB
  outA=$(pawl status taskA 2>/dev/null)
  outB=$(pawl status taskB 2>/dev/null)
  assert_json "$outA" ".status" "completed" || return 1
  assert_json "$outB" ".status" "running" || return 1
  pawl done taskB >/dev/null 2>&1 || true
}

test_vp_events_follow() {
  setup_vp_project "evfollow" '{"workflow":[{"name":"work","run":"sleep 60","in_viewport":true}]}'
  create_task t1
  local events_file="${E2E_TMP}/pawl-e2e-vp-events-follow.jsonl"
  rm -f "$events_file"
  pawl events --follow t1 > "$events_file" 2>/dev/null &
  local follow_pid=$!
  sleep 0.3
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "running" 5 || { kill "$follow_pid" 2>/dev/null; return 1; }
  sleep 0.5
  pawl done t1 >/dev/null 2>&1
  sleep 0.5
  kill "$follow_pid" 2>/dev/null || true
  wait "$follow_pid" 2>/dev/null || true
  [ -s "$events_file" ] || { echo "no events captured"; return 1; }
  assert_contains "$(cat "$events_file")" '"type":"task_started"' || return 1
  assert_contains "$(cat "$events_file")" '"type":"viewport_launched"' || return 1
  rm -f "$events_file"
}

test_vp_last_feedback_propagated() {
  local marker="${E2E_TMP}/pawl-e2e-vp-feedback-marker"
  local counter="${E2E_TMP}/pawl-e2e-vp-feedback-count"
  rm -f "$marker" "$counter"
  setup_vp_project "feedback" "{\"workflow\":[{\"name\":\"build\",\"run\":\"echo \$PAWL_LAST_VERIFY_OUTPUT > $marker\",\"in_viewport\":true,\"verify\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; if [ \$count -lt 2 ]; then echo FEEDBACK_MSG >&2; exit 1; fi\",\"on_fail\":\"retry\",\"max_retries\":3}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  wait_status t1 "completed" 15 || return 1
  [ -f "$marker" ] || { echo "marker file not created"; return 1; }
  local content
  content=$(cat "$marker")
  assert_contains "$content" "FEEDBACK_MSG" || return 1
  rm -f "$marker" "$counter"
}

# ═══════════════════════════════════════════════════════
# Test Registry — label shown in report
# ═══════════════════════════════════════════════════════

TESTS=(
  "test_vp_auto_complete|in_viewport auto-complete (run: true)"
  "test_vp_auto_fail|in_viewport auto-fail (run: false)"
  "test_vp_multi_step|mixed sync + viewport + sync → completed"
  "test_vp_done_completes|done completes long-running viewport step"
  "test_vp_done_with_verify_manual|done with verify=manual → waiting → done → completed"
  "test_vp_done_with_verify_cmd|viewport + verify=true → auto-verify pass → completed"
  "test_vp_retry_auto|viewport retry auto → eventual success"
  "test_vp_retry_exhaustion|viewport retry exhaustion → failed"
  "test_vp_on_fail_manual|viewport on_fail=manual → waiting → done accepts"
  "test_vp_loss_detected|viewport loss → failed"
  "test_vp_loss_recovery|viewport loss → reset --step → re-executes"
  "test_vp_capture_content|capture content contains marker"
  "test_vp_capture_no_viewport|capture pending → viewport_exists=false"
  "test_vp_capture_active|capture active → process_active=true, after done → viewport gone"
  "test_vp_enter|enter viewport → exit 0"
  "test_vp_enter_no_viewport|enter no viewport → exit 4"
  "test_vp_stop|stop running viewport → stopped"
  "test_vp_env_vars|PAWL_ env vars available in viewport"
  "test_vp_consecutive|consecutive in_viewport steps → exec chain"
  "test_vp_consecutive_three|three consecutive in_viewport steps"
  "test_vp_done_message|done -m records message in log"
  "test_vp_done_message_gate|done -m on gate step (sync)"
  "test_vp_skip|skip in_viewport step via frontmatter"
  "test_vp_hook_viewport_launched|hook viewport_launched fires"
  "test_vp_hook_viewport_lost|hook viewport_lost fires"
  "test_vp_verify_fail_retry|viewport verify fail triggers retry"
  "test_vp_task_index|start by task index (1-based)"
  "test_vp_task_index_invalid|start invalid task index → exit 4"
  "test_vp_full_reset_running|full reset while viewport running → pending"
  "test_vp_multi_task|two tasks in same session → isolated windows"
  "test_vp_events_follow|events --follow captures live events"
  "test_vp_last_feedback_propagated|last_feedback available in retry context"
)

# ═══════════════════════════════════════════════════════
# Parallel Runner
# ═══════════════════════════════════════════════════════

echo "── Viewport E2E Tests ──"
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
      # First line of output is the error message
      echo "${output}" > "${RESULTS_DIR}/${func}"
    fi
  ) &
done

wait

# ═══════════════════════════════════════════════════════
# Report (in registration order)
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
