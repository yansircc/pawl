#!/usr/bin/env bash
# Claude Code adapter for pawl — start + read
# Copy to .pawl/drivers/claude.sh and customize.
#
# Config examples:
#   Pipe:  { "run": "cat $PAWL_TASK_FILE | .pawl/drivers/claude.sh", ... }
#   TUI:   { "run": ".pawl/drivers/claude.sh", ... }
#
# Reduce system prompt tokens (~13x):
#   --tools "Bash,Write" --setting-sources "" --mcp-config '{"mcpServers":{}}' --disable-slash-commands
set -euo pipefail

case "${1:-start}" in
  start)
    FLAGS=(--dangerously-skip-permissions)
    [ -t 0 ] || FLAGS+=(-p)

    if [ "${PAWL_RETRY_COUNT:-0}" -gt 0 ]; then
      claude "${FLAGS[@]}" -r "$PAWL_RUN_ID" \
        "Fix: ${PAWL_LAST_VERIFY_OUTPUT:-verify failed}"
    else
      if [ -t 0 ]; then
        claude "${FLAGS[@]}" --session-id "$PAWL_RUN_ID" \
          "$(cat "$PAWL_TASK_FILE")"
      else
        claude "${FLAGS[@]}" --session-id "$PAWL_RUN_ID"
      fi
    fi
    ;;
  read)
    PROJECT_HASH=$(echo "$PAWL_PROJECT_ROOT" | sed 's|^/||; s|/|-|g')
    LOG="$HOME/.claude/projects/-${PROJECT_HASH}/${PAWL_RUN_ID}.jsonl"
    if [ -f "$LOG" ]; then
      cat "$LOG"
    else
      echo "No session log: $LOG" >&2; exit 1
    fi
    ;;
esac
