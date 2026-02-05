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
    // Log-related fields (optional)
    pub log_dir: Option<String>,
    pub log_path: Option<String>,
    pub prev_log: Option<String>,
    pub step_index: Option<usize>,
}

impl Context {
    /// Create a new context for a task (basic, without log info)
    pub fn new(
        task: &str,
        session: &str,
        repo_root: &str,
        worktree_dir: &str,
        step: &str,
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
            log_dir: None,
            log_path: None,
            prev_log: None,
            step_index: None,
        }
    }

    /// Create a full context with log information
    pub fn new_full(
        task: &str,
        session: &str,
        repo_root: &str,
        worktree_dir: &str,
        step: &str,
        step_index: usize,
        log_dir: &str,
        log_path: &str,
        prev_log: Option<&str>,
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
            log_dir: Some(log_dir.to_string()),
            log_path: Some(log_path.to_string()),
            prev_log: prev_log.map(|s| s.to_string()),
            step_index: Some(step_index),
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
            .replace("${step}", &self.step);

        // Expand log-related variables if available
        if let Some(log_dir) = &self.log_dir {
            result = result.replace("${log_dir}", log_dir);
        }
        if let Some(log_path) = &self.log_path {
            result = result.replace("${log_path}", log_path);
        }
        if let Some(prev_log) = &self.prev_log {
            result = result.replace("${prev_log}", prev_log);
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

        // Add log-related environment variables if available
        if let Some(log_dir) = &self.log_dir {
            env.insert("WF_LOG_DIR".to_string(), log_dir.clone());
        }
        if let Some(log_path) = &self.log_path {
            env.insert("WF_LOG_PATH".to_string(), log_path.clone());
        }
        if let Some(prev_log) = &self.prev_log {
            env.insert("WF_PREV_LOG".to_string(), prev_log.clone());
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
        );

        assert_eq!(ctx.expand("${task}"), "auth");
        assert_eq!(ctx.expand("${branch}"), "wf/auth");
        assert_eq!(
            ctx.expand("${worktree}"),
            "/home/user/project/.wf/worktrees/auth"
        );
        assert_eq!(
            ctx.expand("git checkout ${branch}"),
            "git checkout wf/auth"
        );
    }

    #[test]
    fn test_expand_full_context() {
        let ctx = Context::new_full(
            "auth",
            "my-project",
            "/home/user/project",
            ".wf/worktrees",
            "Develop",
            1,
            "/home/user/project/.wf/logs/auth",
            "/home/user/project/.wf/logs/auth/step-2-develop.log",
            Some("/home/user/project/.wf/logs/auth/step-1-setup.log"),
        );

        assert_eq!(
            ctx.expand("${log_dir}"),
            "/home/user/project/.wf/logs/auth"
        );
        assert_eq!(
            ctx.expand("${log_path}"),
            "/home/user/project/.wf/logs/auth/step-2-develop.log"
        );
        assert_eq!(
            ctx.expand("${prev_log}"),
            "/home/user/project/.wf/logs/auth/step-1-setup.log"
        );
        assert_eq!(ctx.expand("${step_index}"), "1");
        assert_eq!(
            ctx.expand("cat ${prev_log}"),
            "cat /home/user/project/.wf/logs/auth/step-1-setup.log"
        );
    }

    #[test]
    fn test_env_vars_full() {
        let ctx = Context::new_full(
            "auth",
            "my-project",
            "/home/user/project",
            ".wf/worktrees",
            "Develop",
            1,
            "/logs",
            "/logs/step.log",
            Some("/logs/prev.log"),
        );

        let env = ctx.to_env_vars();
        assert_eq!(env.get("WF_LOG_DIR"), Some(&"/logs".to_string()));
        assert_eq!(env.get("WF_LOG_PATH"), Some(&"/logs/step.log".to_string()));
        assert_eq!(env.get("WF_PREV_LOG"), Some(&"/logs/prev.log".to_string()));
        assert_eq!(env.get("WF_STEP_INDEX"), Some(&"1".to_string()));
    }
}
