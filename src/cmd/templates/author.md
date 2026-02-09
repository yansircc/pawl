# Author — Writing Task Definitions

Create: `pawl create <name> [description] [--depends a,b]`

Task files (`.pawl/tasks/{task}.md`) serve dual purpose:
1. **pawl definition**: frontmatter (name, depends, skip) controls sequencing
2. **AI worker prompt**: `cat $PAWL_TASK_FILE | <agent-cli>` — the file IS the prompt

Frontmatter fields: `name` (required, matches filename), `depends` (list of prerequisite tasks — **enforced**: `pawl start` refuses with exit 3 if any dependency is not completed), `skip` (list of step names to auto-skip for this task).

## Writing Effective Tasks

### Goal

State the desired outcome, not the steps. Let the worker decide HOW.

- Bad: "Run `npm init`, then install express, then create server.js with..."
- Good: "Create a REST API server with Express that exposes GET /users and POST /users endpoints."

### Constraints

Specify what NOT to do and hard requirements. Workers over-engineer without boundaries.

- Technology choices: "Use SQLite, not Postgres"
- Scope limits: "Only modify files in src/api/"
- Standards: "Follow existing code style, no new dependencies"

### Acceptance Criteria

Map directly to verify commands. Each criterion should be testable.

- Bad: "Code should be clean and well-tested"
- Good:
  - `npm test` passes (all existing + new tests)
  - `curl localhost:3000/users` returns 200 with JSON array
  - No TypeScript errors (`npx tsc --noEmit`)

## On Retry Failure

Append fix guidance to the end of task file — don't overwrite (preserves history):

```markdown
---
## Fix Guidance (Round 2)

Previous issue: test_refresh_token assertion error
Fix: Extract token generation into a pure function, pass fixed time in tests
```

The worker sees full history across retries, avoids repeating mistakes.
