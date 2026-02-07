#!/usr/bin/env bash
# .wf/lib/ai-helpers.sh — AI worker helper functions
# Source this file in your worker/wrapper scripts:
#   source "$(dirname "$0")/../lib/ai-helpers.sh"

set -uo pipefail

# Extract the most recent session_id from a task's JSONL log.
# Usage: extract_session_id <jsonl_file>
# Returns empty string if no session_id found.
extract_session_id() {
    local log_file="${1:?Usage: extract_session_id <jsonl_file>}"
    [ -f "$log_file" ] || { echo ""; return 0; }
    { grep -o '"session_id":"[^"]*"' "$log_file" || true; } | tail -1 | cut -d'"' -f4
}

# Extract the most recent failure feedback from a task's JSONL log.
# Looks for step_completed events with exit_code != 0 and extracts stderr.
# Usage: extract_feedback <jsonl_file> [step_index]
# Returns empty string if no feedback found.
extract_feedback() {
    local log_file="${1:?Usage: extract_feedback <jsonl_file> [step_index]}"
    local step_idx="${2:-}"
    [ -f "$log_file" ] || { echo ""; return 0; }

    if [ -n "$step_idx" ]; then
        { grep '"type":"step_completed"' "$log_file" || true; } \
            | grep "\"step\":${step_idx}" \
            | jq -r 'select(.exit_code != 0) | .stderr // empty' 2>/dev/null \
            | tail -1
    else
        { grep '"type":"step_completed"' "$log_file" || true; } \
            | jq -r 'select(.exit_code != 0) | .stderr // empty' 2>/dev/null \
            | tail -1
    fi
}

# AI worker wrapper: handles fresh start vs resume, injects feedback.
# Usage: run_ai_worker [options]
#   --log-file <path>     JSONL log file (default: $WF_LOG_FILE)
#   --task-file <path>    Task markdown file (default: $WF_TASK_FILE)
#   --tools <tools>       Comma-separated tool list (default: Bash,Read,Write)
#   --claude-cmd <cmd>    Claude command (default: claude)
#   --extra-args <args>   Extra arguments to pass to claude
run_ai_worker() {
    local log_file="${WF_LOG_FILE:-}"
    local task_file="${WF_TASK_FILE:-}"
    local tools="Bash,Read,Write"
    local claude_cmd="${WF_CLAUDE_COMMAND:-claude}"
    local extra_args=""

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --log-file)   log_file="$2"; shift 2 ;;
            --task-file)  task_file="$2"; shift 2 ;;
            --tools)      tools="$2"; shift 2 ;;
            --claude-cmd) claude_cmd="$2"; shift 2 ;;
            --extra-args) extra_args="$2"; shift 2 ;;
            *) echo "Unknown option: $1" >&2; return 1 ;;
        esac
    done

    [ -z "$log_file" ] && { echo "Error: --log-file or WF_LOG_FILE required" >&2; return 1; }
    [ -z "$task_file" ] && { echo "Error: --task-file or WF_TASK_FILE required" >&2; return 1; }

    local session_id
    session_id=$(extract_session_id "$log_file")

    local feedback
    feedback=$(extract_feedback "$log_file")

    if [ -n "$session_id" ]; then
        # Resume existing session with feedback
        local prompt="Continue working on this task."
        [ -n "$feedback" ] && prompt="Previous attempt failed verification. Feedback: ${feedback}. Please fix and try again."
        echo "[ai-helpers] Resuming session ${session_id}" >&2
        $claude_cmd -p "$prompt" -r "$session_id" --tools "$tools" $extra_args
    else
        # Fresh start: pipe task file as prompt
        echo "[ai-helpers] Starting fresh session" >&2
        cat "$task_file" | $claude_cmd -p - --tools "$tools" $extra_args
    fi
}