#!/usr/bin/env bash
# Claude Code driver for pawl — start/send/stop/read
# Copy to .pawl/drivers/claude.sh and customize.
#
# Config example:
#   { "name": "develop", "run": "cat $PAWL_TASK_FILE | .pawl/drivers/claude.sh start",
#     "in_viewport": true, "verify": "<test>", "on_fail": "retry" }
set -euo pipefail

case "${1:?Usage: $0 start|send|stop|read}" in
  start)
    if [ "${PAWL_RETRY_COUNT:-0}" -gt 0 ]; then
      claude -p "Fix: ${PAWL_LAST_VERIFY_OUTPUT:-verify failed}" \
        -r "$PAWL_RUN_ID" \
        --dangerously-skip-permissions
    else
      claude -p \
        --session-id "$PAWL_RUN_ID" \
        --dangerously-skip-permissions
    fi
    ;;
  send)
    shift
    tmux send-keys -t "$PAWL_SESSION:$PAWL_TASK" "$*" Enter
    ;;
  stop)
    tmux send-keys -t "$PAWL_SESSION:$PAWL_TASK" C-c
    ;;
  read)
    project_hash=$(echo "$PAWL_PROJECT_ROOT" | tr '/' '-')
    log="${HOME}/.claude/projects/${project_hash}/${PAWL_RUN_ID}.jsonl"
    if [ -f "$log" ]; then
      cat "$log"
    else
      echo "No session log at $log" >&2
      exit 1
    fi
    ;;
esac
