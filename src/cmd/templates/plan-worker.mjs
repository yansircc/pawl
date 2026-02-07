import { query } from "@anthropic-ai/claude-agent-sdk";
import { randomUUID } from "crypto";
import * as fs from "fs";
import * as path from "path";

const task = process.env.PAWL_TASK;
const taskFile = process.env.PAWL_TASK_FILE;
const repoRoot = process.env.PAWL_REPO_ROOT;
const planDir = path.join(repoRoot, ".pawl", "plans");

fs.mkdirSync(planDir, { recursive: true });

const sessionId = randomUUID();
const prompt = fs.readFileSync(taskFile, "utf-8");

for await (const message of query({
  prompt,
  options: {
    sessionId,
    permissionMode: "plan",
    canUseTool: async (toolName, input) => {
      if (toolName === "ExitPlanMode") {
        fs.writeFileSync(path.join(planDir, `${task}.md`), input.plan || "");
        fs.writeFileSync(path.join(planDir, `${task}.session`), sessionId);
        console.log(`[plan-worker] Plan saved to .pawl/plans/${task}.md`);
        process.exit(0);
      }
      if (toolName === "AskUserQuestion") {
        const answers = {};
        for (const q of input.questions || []) {
          answers[q.question] = q.options?.[0]?.label || "yes";
        }
        return { behavior: "allow", updatedInput: { ...input, answers } };
      }
      return { behavior: "allow", updatedInput: input };
    },
  },
})) {
  // Silently consume stream; process.exit(0) in canUseTool breaks out
}

// If we reach here, AI did not call ExitPlanMode
console.error("[plan-worker] Warning: AI did not produce a plan");
process.exit(1);
