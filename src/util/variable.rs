use std::collections::HashMap;

/// Context for variable expansion
#[derive(Debug, Clone)]
pub struct Context {
    pub task: String,
    pub branch: String,
    pub worktree: String,
    pub window: String,
    pub session: String,
    pub repo_root: String,
    pub step: String,
    pub base_branch: String,
    // Claude CLI command (default: "claude")
    pub claude_command: String,
    // Log file path (single JSONL file per task)
    pub log_file: Option<String>,
    // Task file path
    pub task_file: Option<String>,
    // Step index (0-based)
    pub step_index: Option<usize>,
}

impl Context {
    /// Create a new context for a task.
    /// Pass `None` for step_index/log_file/task_file when not in a step execution context.
    pub fn new(
        task: &str,
        session: &str,
        repo_root: &str,
        worktree_dir: &str,
        step: &str,
        base_branch: &str,
        claude_command: &str,
        step_index: Option<usize>,
        log_file: Option<&str>,
        task_file: Option<&str>,
    ) -> Self {
        let worktree = format!("{}/{}/{}", repo_root, worktree_dir, task);
        Self {
            task: task.to_string(),
            branch: format!("wf/{}", task),
            worktree,
            window: task.to_string(),
            session: session.to_string(),
            repo_root: repo_root.to_string(),
            step: step.to_string(),
            base_branch: base_branch.to_string(),
            claude_command: claude_command.to_string(),
            log_file: log_file.map(|s| s.to_string()),
            task_file: task_file.map(|s| s.to_string()),
            step_index,
        }
    }

    /// Expand variables in a template string
    pub fn expand(&self, template: &str) -> String {
        let mut result = template
            .replace("${task}", &self.task)
            .replace("${branch}", &self.branch)
            .replace("${worktree}", &self.worktree)
            .replace("${window}", &self.window)
            .replace("${session}", &self.session)
            .replace("${repo_root}", &self.repo_root)
            .replace("${step}", &self.step)
            .replace("${base_branch}", &self.base_branch)
            .replace("${claude_command}", &self.claude_command);

        // Expand log-related variables if available
        if let Some(log_file) = &self.log_file {
            result = result.replace("${log_file}", log_file);
        }
        if let Some(task_file) = &self.task_file {
            result = result.replace("${task_file}", task_file);
        }
        if let Some(step_index) = self.step_index {
            result = result.replace("${step_index}", &step_index.to_string());
        }

        result
    }

    /// Convert to environment variables for subprocess
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        env.insert("WF_TASK".to_string(), self.task.clone());
        env.insert("WF_BRANCH".to_string(), self.branch.clone());
        env.insert("WF_WORKTREE".to_string(), self.worktree.clone());
        env.insert("WF_WINDOW".to_string(), self.window.clone());
        env.insert("WF_SESSION".to_string(), self.session.clone());
        env.insert("WF_REPO_ROOT".to_string(), self.repo_root.clone());
        env.insert("WF_STEP".to_string(), self.step.clone());
        env.insert("WF_BASE_BRANCH".to_string(), self.base_branch.clone());
        env.insert("WF_CLAUDE_COMMAND".to_string(), self.claude_command.clone());

        // Add log-related environment variables if available
        if let Some(log_file) = &self.log_file {
            env.insert("WF_LOG_FILE".to_string(), log_file.clone());
        }
        if let Some(task_file) = &self.task_file {
            env.insert("WF_TASK_FILE".to_string(), task_file.clone());
        }
        if let Some(step_index) = self.step_index {
            env.insert("WF_STEP_INDEX".to_string(), step_index.to_string());
        }

        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand() {
        let ctx = Context::new(
            "auth",
            "my-project",
            "/home/user/project",
            ".wf/worktrees",
            "Type check",
            "main",
            "claude",
            None,
            None,
            None,
        );

        assert_eq!(ctx.expand("${task}"), "auth");
        assert_eq!(ctx.expand("${branch}"), "wf/auth");
        assert_eq!(ctx.expand("${base_branch}"), "main");
        assert_eq!(
            ctx.expand("${worktree}"),
            "/home/user/project/.wf/worktrees/auth"
        );
        assert_eq!(
            ctx.expand("git checkout ${branch}"),
            "git checkout wf/auth"
        );
        assert_eq!(
            ctx.expand("git branch ${branch} ${base_branch}"),
            "git branch wf/auth main"
        );
    }

    #[test]
    fn test_expand_full_context() {
        let ctx = Context::new(
            "auth",
            "my-project",
            "/home/user/project",
            ".wf/worktrees",
            "Develop",
            "develop",
            "claude",
            Some(1),
            Some("/home/user/project/.wf/logs/auth.jsonl"),
            Some("/home/user/project/.wf/tasks/auth.md"),
        );

        assert_eq!(
            ctx.expand("${log_file}"),
            "/home/user/project/.wf/logs/auth.jsonl"
        );
        assert_eq!(
            ctx.expand("${task_file}"),
            "/home/user/project/.wf/tasks/auth.md"
        );
        assert_eq!(ctx.expand("${step_index}"), "1");
        assert_eq!(ctx.expand("${base_branch}"), "develop");
        assert_eq!(
            ctx.expand("cat ${log_file}"),
            "cat /home/user/project/.wf/logs/auth.jsonl"
        );
    }

    #[test]
    fn test_env_vars_full() {
        let ctx = Context::new(
            "auth",
            "my-project",
            "/home/user/project",
            ".wf/worktrees",
            "Develop",
            "main",
            "ccc",
            Some(1),
            Some("/logs/auth.jsonl"),
            Some("/tasks/auth.md"),
        );

        let env = ctx.to_env_vars();
        assert_eq!(env.get("WF_LOG_FILE"), Some(&"/logs/auth.jsonl".to_string()));
        assert_eq!(env.get("WF_TASK_FILE"), Some(&"/tasks/auth.md".to_string()));
        assert_eq!(env.get("WF_STEP_INDEX"), Some(&"1".to_string()));
        assert_eq!(env.get("WF_BASE_BRANCH"), Some(&"main".to_string()));
        assert_eq!(env.get("WF_CLAUDE_COMMAND"), Some(&"ccc".to_string()));
    }

    #[test]
    fn test_env_vars_basic() {
        let ctx = Context::new(
            "auth",
            "my-project",
            "/home/user/project",
            ".wf/worktrees",
            "Setup",
            "main",
            "claude",
            None,
            None,
            None,
        );

        let env = ctx.to_env_vars();
        assert_eq!(env.get("WF_TASK"), Some(&"auth".to_string()));
        assert_eq!(env.get("WF_BRANCH"), Some(&"wf/auth".to_string()));
        assert_eq!(env.get("WF_BASE_BRANCH"), Some(&"main".to_string()));
        assert!(env.get("WF_LOG_FILE").is_none());
        assert!(env.get("WF_TASK_FILE").is_none());
    }
}
