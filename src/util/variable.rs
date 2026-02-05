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
}

impl Context {
    /// Create a new context for a task
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
        }
    }

    /// Expand variables in a template string
    pub fn expand(&self, template: &str) -> String {
        template
            .replace("${task}", &self.task)
            .replace("${branch}", &self.branch)
            .replace("${worktree}", &self.worktree)
            .replace("${window}", &self.window)
            .replace("${session}", &self.session)
            .replace("${repo_root}", &self.repo_root)
            .replace("${step}", &self.step)
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
        env
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand() {
        let ctx = Context::new("auth", "my-project", "/home/user/project", ".wf/worktrees", "Type check");

        assert_eq!(ctx.expand("${task}"), "auth");
        assert_eq!(ctx.expand("${branch}"), "wf/auth");
        assert_eq!(ctx.expand("${worktree}"), "/home/user/project/.wf/worktrees/auth");
        assert_eq!(ctx.expand("git checkout ${branch}"), "git checkout wf/auth");
    }
}
