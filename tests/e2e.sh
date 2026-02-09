#!/usr/bin/env bash
# pawl E2E tests — exercises the CLI binary end-to-end.
# Prerequisites: pawl (cargo install --path .), jq
# Usage: bash tests/e2e.sh

set -euo pipefail

# ── Resolve /tmp for macOS (/tmp → /private/tmp) ──
E2E_TMP="$(cd /tmp && pwd -P)"

# ── Counters ──
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0
FAILURES=""

# ── Helpers ──

begin_test() {
  TESTS_RUN=$((TESTS_RUN + 1))
  CURRENT_TEST="$1"
  echo -n "  $1 ... "
}

pass() {
  TESTS_PASSED=$((TESTS_PASSED + 1))
  echo "ok"
}

fail() {
  TESTS_FAILED=$((TESTS_FAILED + 1))
  local msg="${1:-}"
  FAILURES="${FAILURES}\n  FAIL: ${CURRENT_TEST}${msg:+ — $msg}"
  echo "FAIL${msg:+ — $msg}"
}

assert_exit() {
  local expected="$1" actual="$2"
  if [ "$expected" != "$actual" ]; then
    fail "expected exit $expected, got $actual"
    return 1
  fi
  return 0
}

assert_json() {
  local json="$1" expr="$2" expected="$3"
  local actual
  actual=$(echo "$json" | jq -r "$expr" 2>/dev/null) || { fail "jq parse error"; return 1; }
  if [ "$actual" != "$expected" ]; then
    fail "jq '$expr' expected '$expected', got '$actual'"
    return 1
  fi
  return 0
}

assert_json_num() {
  local json="$1" expr="$2" expected="$3"
  local actual
  actual=$(echo "$json" | jq "$expr" 2>/dev/null) || { fail "jq parse error"; return 1; }
  if [ "$actual" != "$expected" ]; then
    fail "jq '$expr' expected $expected, got $actual"
    return 1
  fi
  return 0
}

assert_contains() {
  local haystack="$1" needle="$2"
  if [[ "$haystack" != *"$needle"* ]]; then
    fail "expected to contain '$needle'"
    return 1
  fi
  return 0
}

assert_not_contains() {
  local haystack="$1" needle="$2"
  if [[ "$haystack" == *"$needle"* ]]; then
    fail "expected NOT to contain '$needle'"
    return 1
  fi
  return 0
}

# Setup a project in /tmp with a given config
# Usage: setup_project <test-name> '<config-json>'
setup_project() {
  local name="$1" config="$2"
  local dir="${E2E_TMP}/pawl-e2e-${name}"
  rm -rf "$dir"
  mkdir -p "$dir"
  cd "$dir"
  pawl init >/dev/null 2>&1
  echo "$config" > .pawl/config.json
}

# Add a task entry to config.json
# Usage: create_task <name> [json-opts]
# Examples: create_task t1
#           create_task child '{"depends":["parent"]}'
#           create_task t1 '{"skip":["dangerous"]}'
create_task() {
  local name="$1"
  local empty='{}'
  local opts="${2:-$empty}"
  python3 -c "import json,sys; c=json.load(open('.pawl/config.json')); c.setdefault('tasks',{})[sys.argv[1]]=json.loads(sys.argv[2]); json.dump(c,open('.pawl/config.json','w'),indent=2)" "$name" "$opts"
}

cleanup() {
  rm -rf ${E2E_TMP}/pawl-e2e-*
}

report() {
  echo ""
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo "  Results: $TESTS_PASSED/$TESTS_RUN passed, $TESTS_FAILED failed"
  if [ $TESTS_FAILED -gt 0 ]; then
    echo -e "$FAILURES"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    exit 1
  fi
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  exit 0
}

# ═══════════════════════════════════════════════════════
# 1. Lifecycle Basics
# ═══════════════════════════════════════════════════════
echo "── Lifecycle Basics ──"

test_init() {
  begin_test "init creates .pawl/ structure"
  local dir="${E2E_TMP}/pawl-e2e-init"
  rm -rf "$dir" && mkdir -p "$dir" && cd "$dir"
  local out
  out=$(pawl init 2>/dev/null)
  assert_json "$out" ".pawl_dir" "$dir/.pawl" || return
  [ -f .pawl/config.json ] || { fail "missing config.json"; return; }
  [ -f .pawl/README.md ] || { fail "missing README.md"; return; }
  pass
}

test_init_duplicate() {
  begin_test "init duplicate → exit 5"
  local dir="${E2E_TMP}/pawl-e2e-init-dup"
  rm -rf "$dir" && mkdir -p "$dir" && cd "$dir"
  pawl init >/dev/null 2>&1
  local rc=0
  pawl init >/dev/null 2>&1 || rc=$?
  assert_exit 5 "$rc" || return
  pass
}

test_start_and_complete() {
  begin_test "start single-step → completed"
  setup_project "start1" '{"workflow":[{"name":"build","run":"true"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  assert_json_num "$out" ".current_step" "1" || return
  pass
}

test_init
test_init_duplicate
test_start_and_complete

# ═══════════════════════════════════════════════════════
# 2. Multi-step & Edge Cases
# ═══════════════════════════════════════════════════════
echo "── Multi-step & Edge Cases ──"

test_start_multi_step() {
  begin_test "start two steps → completed"
  setup_project "multi" '{"workflow":[{"name":"a","run":"true"},{"name":"b","run":"true"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  assert_json_num "$out" ".current_step" "2" || return
  pass
}

test_start_no_project() {
  begin_test "start with no .pawl/ → exit 4"
  local dir="${E2E_TMP}/pawl-e2e-noproj"
  rm -rf "$dir" && mkdir -p "$dir" && cd "$dir"
  local rc=0
  pawl start foo >/dev/null 2>&1 || rc=$?
  assert_exit 4 "$rc" || return
  pass
}

test_start_undeclared_task() {
  begin_test "start undeclared task → completes (no constraints)"
  setup_project "notask" '{"workflow":[{"name":"a","run":"true"}]}'
  local out
  out=$(pawl start nonexistent 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_start_already_waiting() {
  begin_test "start while waiting → exit 2"
  setup_project "waitstart" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl start t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_start_completed() {
  begin_test "start when completed → exit 2"
  setup_project "compstart" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl start t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_start_reset_flag() {
  begin_test "start --reset re-runs"
  setup_project "resetflag" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl start --reset t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_start_multi_step
test_start_no_project
test_start_undeclared_task
test_start_already_waiting
test_start_completed
test_start_reset_flag

# ═══════════════════════════════════════════════════════
# 3. Gate Steps
# ═══════════════════════════════════════════════════════
echo "── Gate Steps ──"

test_gate_yields() {
  begin_test "gate step → waiting/gate"
  setup_project "gate1" '{"workflow":[{"name":"approve"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "waiting" || return
  assert_json "$out" ".message" "gate" || return
  pass
}

test_gate_done_continues() {
  begin_test "gate done → continues and completes"
  setup_project "gate2" '{"workflow":[{"name":"approve"},{"name":"build","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_gate_at_end() {
  begin_test "gate as last step → done completes"
  setup_project "gate3" '{"workflow":[{"name":"build","run":"true"},{"name":"approve"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_gate_yields
test_gate_done_continues
test_gate_at_end

# ═══════════════════════════════════════════════════════
# 4. Verify Manual
# ═══════════════════════════════════════════════════════
echo "── Verify Manual ──"

test_verify_manual_yields() {
  begin_test "verify=manual → waiting/verify_manual"
  setup_project "vman1" '{"workflow":[{"name":"build","run":"true","verify":"manual"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "waiting" || return
  assert_json "$out" ".message" "verify_manual" || return
  pass
}

test_verify_manual_done_completes() {
  begin_test "verify=manual done → completed"
  setup_project "vman2" '{"workflow":[{"name":"build","run":"true","verify":"manual"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_verify_manual_yields
test_verify_manual_done_completes

# ═══════════════════════════════════════════════════════
# 5. Verify Command
# ═══════════════════════════════════════════════════════
echo "── Verify Command ──"

test_verify_command_pass() {
  begin_test "verify command pass → completed"
  setup_project "vcmd1" '{"workflow":[{"name":"build","run":"true","verify":"true"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_verify_command_fail_terminal() {
  begin_test "verify fail (no on_fail) → failed"
  setup_project "vcmd2" '{"workflow":[{"name":"build","run":"true","verify":"false"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "failed" || return
  pass
}

test_verify_fail_captures_output() {
  begin_test "verify fail → last_feedback populated"
  setup_project "vcmd3" '{"workflow":[{"name":"build","run":"true","verify":"echo VERIFY_FAIL_MSG >&2; false"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>/dev/null
  local status_out
  status_out=$(pawl status t1 2>/dev/null)
  assert_contains "$(echo "$status_out" | jq -r '.last_feedback // empty')" "VERIFY_FAIL_MSG" || return
  pass
}

test_verify_command_pass
test_verify_command_fail_terminal
test_verify_fail_captures_output

# ═══════════════════════════════════════════════════════
# 6. on_fail retry
# ═══════════════════════════════════════════════════════
echo "── on_fail retry ──"

test_retry_eventual_success() {
  begin_test "retry → eventual success"
  local counter="${E2E_TMP}/pawl-e2e-retry-count"
  rm -f "$counter"
  # Command: first call fails (file doesn't exist yet or count < 2), second succeeds
  setup_project "retry1" "{\"workflow\":[{\"name\":\"build\",\"run\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\",\"on_fail\":\"retry\",\"max_retries\":3}]}"
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  rm -f "$counter"
  pass
}

test_retry_exhaustion() {
  begin_test "retry exhaustion → failed"
  setup_project "retry2" '{"workflow":[{"name":"build","run":"false","on_fail":"retry","max_retries":2}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "failed" || return
  pass
}

test_retry_verify_fail() {
  begin_test "retry on verify failure"
  local counter="${E2E_TMP}/pawl-e2e-retry-verify-count"
  rm -f "$counter"
  setup_project "retry3" "{\"workflow\":[{\"name\":\"build\",\"run\":\"true\",\"verify\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\",\"on_fail\":\"retry\",\"max_retries\":3}]}"
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  rm -f "$counter"
  pass
}

test_retry_eventual_success
test_retry_exhaustion
test_retry_verify_fail

# ═══════════════════════════════════════════════════════
# 7. on_fail manual
# ═══════════════════════════════════════════════════════
echo "── on_fail manual ──"

test_on_fail_manual_yields() {
  begin_test "on_fail=manual → waiting/on_fail_manual"
  setup_project "ofm1" '{"workflow":[{"name":"build","run":"false","on_fail":"manual"}]}'
  create_task t1
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "waiting" || return
  assert_json "$out" ".message" "on_fail_manual" || return
  pass
}

test_on_fail_manual_done_accepts() {
  begin_test "on_fail=manual done → accepts and continues"
  setup_project "ofm2" '{"workflow":[{"name":"build","run":"false","on_fail":"manual"},{"name":"fin","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl done t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_on_fail_manual_reset_step() {
  begin_test "on_fail=manual reset --step → retries"
  local counter="${E2E_TMP}/pawl-e2e-ofm-reset-count"
  rm -f "$counter"
  setup_project "ofm3" "{\"workflow\":[{\"name\":\"build\",\"run\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\",\"on_fail\":\"manual\"}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl reset --step t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  rm -f "$counter"
  pass
}

test_on_fail_manual_yields
test_on_fail_manual_done_accepts
test_on_fail_manual_reset_step

# ═══════════════════════════════════════════════════════
# 8. Skip
# ═══════════════════════════════════════════════════════
echo "── Skip ──"

test_skip_step() {
  begin_test "skip step via config tasks"
  setup_project "skip1" '{"workflow":[{"name":"dangerous","run":"false"},{"name":"fin","run":"true"}]}'
  create_task t1 '{"skip":["dangerous"]}'
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_skip_multiple() {
  begin_test "skip multiple steps"
  setup_project "skip2" '{"workflow":[{"name":"a","run":"false"},{"name":"b","run":"false"},{"name":"c","run":"true"}]}'
  create_task t1 '{"skip":["a","b"]}'
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_skip_step
test_skip_multiple

# ═══════════════════════════════════════════════════════
# 9. Stop
# ═══════════════════════════════════════════════════════
echo "── Stop ──"

test_stop_waiting() {
  begin_test "stop waiting → stopped"
  setup_project "stop1" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl stop t1 2>/dev/null)
  assert_json "$out" ".status" "stopped" || return
  pass
}

test_stop_pending() {
  begin_test "stop pending → exit 2"
  setup_project "stop2" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  local rc=0
  pawl stop t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_stop_completed() {
  begin_test "stop completed → exit 2"
  setup_project "stop3" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl stop t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_stop_waiting
test_stop_pending
test_stop_completed

# ═══════════════════════════════════════════════════════
# 10. Reset
# ═══════════════════════════════════════════════════════
echo "── Reset ──"

test_full_reset() {
  begin_test "full reset → pending"
  setup_project "reset1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl reset t1 2>/dev/null)
  assert_json "$out" ".status" "pending" || return
  pass
}

test_step_reset_failed() {
  begin_test "step reset failed → retries and succeeds"
  local counter="${E2E_TMP}/pawl-e2e-stepreset-count"
  rm -f "$counter"
  setup_project "reset2" "{\"workflow\":[{\"name\":\"build\",\"run\":\"count=\$(cat $counter 2>/dev/null || echo 0); count=\$((count+1)); echo \$count > $counter; [ \$count -ge 2 ]\"}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl reset --step t1 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  rm -f "$counter"
  pass
}

test_step_reset_waiting() {
  begin_test "step reset waiting gate → re-enters gate"
  setup_project "reset3" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl reset --step t1 2>/dev/null)
  assert_json "$out" ".status" "waiting" || return
  assert_json "$out" ".message" "gate" || return
  pass
}

test_step_reset_completed() {
  begin_test "step reset completed → exit 2"
  setup_project "reset4" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl reset --step t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_step_reset_pending() {
  begin_test "step reset pending → exit 2"
  setup_project "reset5" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  local rc=0
  pawl reset --step t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_full_reset
test_step_reset_failed
test_step_reset_waiting
test_step_reset_completed
test_step_reset_pending

# ═══════════════════════════════════════════════════════
# 11. Start After Stop/Fail
# ═══════════════════════════════════════════════════════
echo "── Start After Stop/Fail ──"

test_start_after_stop() {
  begin_test "start after stop → re-executes"
  setup_project "restop" '{"workflow":[{"name":"gate"},{"name":"fin","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  pawl stop t1 >/dev/null 2>&1
  # start after stop should work (start auto-resets stopped tasks)
  local out
  out=$(pawl start t1 2>/dev/null)
  # Stopped tasks: start.rs allows Running/Waiting/Completed to block, but Stopped/Failed fall through
  # So start emits a new TaskStarted and re-executes
  assert_json "$out" ".status" "waiting" || return
  assert_json "$out" ".message" "gate" || return
  pass
}

test_start_after_fail() {
  begin_test "start after fail → re-executes"
  setup_project "refail" '{"workflow":[{"name":"build","run":"false"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  # Failed state: start.rs doesn't block it — falls through to new TaskStarted
  local out
  out=$(pawl start t1 2>/dev/null)
  assert_json "$out" ".status" "failed" || return
  pass
}

test_start_after_stop
test_start_after_fail

# ═══════════════════════════════════════════════════════
# 12. Dependencies
# ═══════════════════════════════════════════════════════
echo "── Dependencies ──"

test_dep_blocks_start() {
  begin_test "dependency blocks start → exit 3"
  setup_project "dep1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task parent
  create_task child '{"depends":["parent"]}'
  local rc=0
  pawl start child >/dev/null 2>&1 || rc=$?
  assert_exit 3 "$rc" || return
  pass
}

test_dep_satisfied() {
  begin_test "dependency satisfied → starts"
  setup_project "dep2" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task parent
  create_task child '{"depends":["parent"]}'
  pawl start parent >/dev/null 2>&1
  local out
  out=$(pawl start child 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_dep_chain() {
  begin_test "dependency chain C→B→A"
  setup_project "dep3" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task taskA
  create_task taskB '{"depends":["taskA"]}'
  create_task taskC '{"depends":["taskB"]}'
  # C blocked by B (which is blocked by A)
  local rc=0
  pawl start taskC >/dev/null 2>&1 || rc=$?
  assert_exit 3 "$rc" || return
  # Complete A, B still blocked? No — B depends on A, not on B being complete
  pawl start taskA >/dev/null 2>&1
  pawl start taskB >/dev/null 2>&1
  local out
  out=$(pawl start taskC 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_dep_blocks_start
test_dep_satisfied
test_dep_chain

# ═══════════════════════════════════════════════════════
# 13. Variables
# ═══════════════════════════════════════════════════════
echo "── Variables ──"

test_intrinsic_vars() {
  begin_test "intrinsic vars expand in commands"
  local marker="${E2E_TMP}/pawl-e2e-vars-intrinsic"
  rm -f "$marker"
  setup_project "vars1" "{\"workflow\":[{\"name\":\"check\",\"run\":\"echo \${task}:\${step}:\${step_index} > $marker\"}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local content
  content=$(cat "$marker")
  assert_contains "$content" "t1:check:0" || return
  rm -f "$marker"
  pass
}

test_env_vars() {
  begin_test "PAWL_ env vars available"
  local marker="${E2E_TMP}/pawl-e2e-vars-env"
  rm -f "$marker"
  setup_project "vars2" "{\"workflow\":[{\"name\":\"check\",\"run\":\"echo \$PAWL_TASK:\$PAWL_STEP > $marker\"}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local content
  content=$(cat "$marker")
  assert_contains "$content" "t1:check" || return
  rm -f "$marker"
  pass
}

test_config_vars() {
  begin_test "config.vars defined and expand"
  local marker="${E2E_TMP}/pawl-e2e-vars-config"
  rm -f "$marker"
  setup_project "vars3" "{\"vars\":{\"myvar\":\"hello\",\"combo\":\"\${myvar}-world\"},\"workflow\":[{\"name\":\"check\",\"run\":\"echo \${combo} > $marker\"}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local content
  content=$(cat "$marker")
  assert_contains "$content" "hello-world" || return
  rm -f "$marker"
  pass
}

test_config_vars_as_env() {
  begin_test "config.vars as PAWL_ env vars"
  local marker="${E2E_TMP}/pawl-e2e-vars-configenv"
  rm -f "$marker"
  setup_project "vars4" "{\"vars\":{\"myvar\":\"envtest\"},\"workflow\":[{\"name\":\"check\",\"run\":\"echo \$PAWL_MYVAR > $marker\"}]}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local content
  content=$(cat "$marker")
  assert_contains "$content" "envtest" || return
  rm -f "$marker"
  pass
}

test_intrinsic_vars
test_env_vars
test_config_vars
test_config_vars_as_env

# ═══════════════════════════════════════════════════════
# 14. Event Hooks
# ═══════════════════════════════════════════════════════
echo "── Event Hooks ──"

test_hook_task_started() {
  begin_test "hook task_started fires"
  local marker="${E2E_TMP}/pawl-e2e-hook-started"
  rm -f "$marker"
  setup_project "hook1" "{\"workflow\":[{\"name\":\"a\",\"run\":\"true\"}],\"on\":{\"task_started\":\"touch $marker\"}}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  sleep 0.5
  [ -f "$marker" ] || { fail "hook marker not found"; return; }
  rm -f "$marker"
  pass
}

test_hook_step_finished() {
  begin_test "hook step_finished with vars"
  local marker="${E2E_TMP}/pawl-e2e-hook-finished"
  rm -f "$marker"
  setup_project "hook2" "{\"workflow\":[{\"name\":\"a\",\"run\":\"true\"}],\"on\":{\"step_finished\":\"echo \${success}:\${exit_code} > $marker\"}}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  sleep 0.5
  [ -f "$marker" ] || { fail "hook marker not found"; return; }
  local content
  content=$(cat "$marker")
  assert_contains "$content" "true:0" || return
  rm -f "$marker"
  pass
}

test_hook_step_yielded() {
  begin_test "hook step_yielded with reason"
  local marker="${E2E_TMP}/pawl-e2e-hook-yielded"
  rm -f "$marker"
  setup_project "hook3" "{\"workflow\":[{\"name\":\"gate\"}],\"on\":{\"step_yielded\":\"echo \${reason} > $marker\"}}"
  create_task t1
  pawl start t1 >/dev/null 2>&1
  sleep 0.5
  [ -f "$marker" ] || { fail "hook marker not found"; return; }
  local content
  content=$(cat "$marker")
  assert_contains "$content" "gate" || return
  rm -f "$marker"
  pass
}

test_hook_task_started
test_hook_step_finished
test_hook_step_yielded

# ═══════════════════════════════════════════════════════
# 15. Error Cases
# ═══════════════════════════════════════════════════════
echo "── Error Cases ──"

test_done_pending() {
  begin_test "done pending → exit 2"
  setup_project "donep" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  local rc=0
  pawl done t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_done_completed() {
  begin_test "done completed → exit 2"
  setup_project "donec" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl done t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_done_failed() {
  begin_test "done failed → exit 2"
  setup_project "donef" '{"workflow":[{"name":"a","run":"false"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl done t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_done_stopped() {
  begin_test "done stopped → exit 2"
  setup_project "dones" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  pawl stop t1 >/dev/null 2>&1
  local rc=0
  pawl done t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_log_pending() {
  begin_test "log pending → exit 2"
  setup_project "logp" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  local rc=0
  pawl log t1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_done_pending
test_done_completed
test_done_failed
test_done_stopped
test_log_pending

# ═══════════════════════════════════════════════════════
# 16. Log & Events
# ═══════════════════════════════════════════════════════
echo "── Log & Events ──"

test_log_default() {
  begin_test "log default → last event"
  setup_project "log1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl log t1 2>/dev/null)
  # Last event should be step_finished (success) — only one line
  local lines
  lines=$(echo "$out" | wc -l | tr -d ' ')
  [ "$lines" = "1" ] || { fail "expected 1 line, got $lines"; return; }
  assert_contains "$out" '"type":"step_finished"' || return
  pass
}

test_log_all() {
  begin_test "log --all → current run events"
  setup_project "log2" '{"workflow":[{"name":"a","run":"true"},{"name":"b","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl log --all t1 2>/dev/null)
  # Should have: task_started + step_finished(a) + step_finished(b) = 3 events
  local lines
  lines=$(echo "$out" | wc -l | tr -d ' ')
  [ "$lines" = "3" ] || { fail "expected 3 lines, got $lines"; return; }
  assert_contains "$out" '"type":"task_started"' || return
  pass
}



test_log_step_filter() {
  begin_test "log --step 0 → filters to step 0"
  setup_project "log4" '{"workflow":[{"name":"a","run":"true"},{"name":"b","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl log --all --step 0 t1 2>/dev/null)
  # Only step 0 events: step_finished for step 0
  local lines
  lines=$(echo "$out" | wc -l | tr -d ' ')
  [ "$lines" = "1" ] || { fail "expected 1 line, got $lines"; return; }
  assert_contains "$out" '"step":0' || return
  pass
}

test_events_output() {
  begin_test "events → JSONL with name field"
  setup_project "ev1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl events t1 2>/dev/null)
  # Each line should have "name" field
  local first_line
  first_line=$(echo "$out" | head -1)
  assert_contains "$first_line" '"name":"t1"' || return
  assert_contains "$first_line" '"type"' || return
  pass
}

test_events_type_filter() {
  begin_test "events --type step_finished → filters"
  setup_project "ev2" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl events --type step_finished t1 2>/dev/null)
  local lines
  lines=$(echo "$out" | wc -l | tr -d ' ')
  [ "$lines" = "1" ] || { fail "expected 1 line, got $lines"; return; }
  assert_contains "$out" '"type":"step_finished"' || return
  # Should NOT contain task_started
  assert_not_contains "$out" '"type":"task_started"' || return
  pass
}

test_log_default
test_log_all
test_log_step_filter
test_events_output
test_events_type_filter

# ═══════════════════════════════════════════════════════
# 17. Status & List
# ═══════════════════════════════════════════════════════
echo "── Status & List ──"

test_status_detail() {
  begin_test "status detail → full JSON structure"
  setup_project "stat1" '{"workflow":[{"name":"a","run":"true"},{"name":"b","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".name" "t1" || return
  assert_json "$out" ".status" "completed" || return
  assert_json_num "$out" ".current_step" "2" || return
  assert_json_num "$out" ".total_steps" "2" || return
  # Check workflow array
  assert_json "$out" ".workflow[0].name" "a" || return
  assert_json "$out" ".workflow[0].status" "success" || return
  assert_json "$out" ".workflow[1].name" "b" || return
  assert_json "$out" ".workflow[1].status" "success" || return
  pass
}

test_status_routing_pending() {
  begin_test "status routing: pending → suggest start"
  setup_project "statr1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".suggest[0]" "pawl start t1" || return
  pass
}

test_status_routing_waiting_gate() {
  begin_test "status routing: waiting/gate → prompt"
  setup_project "statr2" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_contains "$(echo "$out" | jq -r '.prompt // empty')" "pawl done t1" || return
  pass
}

test_status_routing_waiting_verify() {
  begin_test "status routing: waiting/verify_manual → prompt"
  setup_project "statr3" '{"workflow":[{"name":"build","run":"true","verify":"manual"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_contains "$(echo "$out" | jq -r '.prompt // empty')" "pawl done t1" || return
  pass
}

test_status_routing_waiting_onfail() {
  begin_test "status routing: waiting/on_fail_manual → suggest+prompt"
  setup_project "statr4" '{"workflow":[{"name":"build","run":"false","on_fail":"manual"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".suggest[0]" "pawl reset --step t1" || return
  assert_contains "$(echo "$out" | jq -r '.prompt // empty')" "pawl done t1" || return
  pass
}

test_status_routing_failed() {
  begin_test "status routing: failed → suggest reset --step"
  setup_project "statr5" '{"workflow":[{"name":"build","run":"false"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".suggest[0]" "pawl reset --step t1" || return
  pass
}

test_status_routing_stopped() {
  begin_test "status routing: stopped → suggest start/reset"
  setup_project "statr6" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  pawl stop t1 >/dev/null 2>&1
  local out
  out=$(pawl status t1 2>/dev/null)
  assert_json "$out" ".suggest[0]" "pawl start t1" || return
  assert_json "$out" ".suggest[1]" "pawl reset t1" || return
  pass
}

test_list() {
  begin_test "list → array of tasks"
  setup_project "list1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task taskA
  create_task taskB
  local out
  out=$(pawl list 2>/dev/null)
  local count
  count=$(echo "$out" | jq 'length')
  [ "$count" = "2" ] || { fail "expected 2 tasks, got $count"; return; }
  pass
}

test_list_empty() {
  begin_test "list empty → []"
  setup_project "list2" '{"workflow":[{"name":"a","run":"true"}]}'
  local out
  out=$(pawl list 2>/dev/null)
  assert_json "$out" "length" "0" || return
  pass
}

test_status_detail
test_status_routing_pending
test_status_routing_waiting_gate
test_status_routing_waiting_verify
test_status_routing_waiting_onfail
test_status_routing_failed
test_status_routing_stopped
test_list
test_list_empty

# ═══════════════════════════════════════════════════════
# 18. Wait
# ═══════════════════════════════════════════════════════
echo "── Wait ──"

test_wait_already_reached() {
  begin_test "wait already reached → immediate return"
  setup_project "wait1" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl wait t1 --until completed -t 2 2>/dev/null)
  assert_json "$out" ".status" "completed" || return
  pass
}

test_wait_terminal_mismatch() {
  begin_test "wait terminal mismatch → exit 2"
  setup_project "wait2" '{"workflow":[{"name":"a","run":"true"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl wait t1 --until running -t 1 >/dev/null 2>&1 || rc=$?
  assert_exit 2 "$rc" || return
  pass
}

test_wait_timeout() {
  begin_test "wait timeout → exit 7"
  setup_project "wait3" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local rc=0
  pawl wait t1 --until completed -t 1 >/dev/null 2>&1 || rc=$?
  assert_exit 7 "$rc" || return
  pass
}

test_wait_multi_status() {
  begin_test "wait multi-status → matches any"
  setup_project "wait4" '{"workflow":[{"name":"gate"}]}'
  create_task t1
  pawl start t1 >/dev/null 2>&1
  local out
  out=$(pawl wait t1 --until "waiting,completed" -t 2 2>/dev/null)
  assert_json "$out" ".status" "waiting" || return
  pass
}

test_wait_already_reached
test_wait_terminal_mismatch
test_wait_timeout
test_wait_multi_status

# ═══════════════════════════════════════════════════════
# Cleanup and Report
# ═══════════════════════════════════════════════════════
cleanup
report
