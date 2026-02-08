use std::collections::HashMap;

/// Context for variable expansion (builder pattern)
#[derive(Debug, Clone)]
pub struct Context {
    vars: Vec<(&'static str, String)>,
}

impl Context {
    pub fn build() -> Self {
        Self { vars: Vec::new() }
    }

    pub fn var(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.vars.push((name, value.into()));
        self
    }

    /// Look up a variable by name
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.iter().rev().find(|(k, _)| *k == key).map(|(_, v)| v.as_str())
    }

    /// Expand variables in a template string
    pub fn expand(&self, template: &str) -> String {
        self.vars.iter().fold(template.to_string(), |s, (k, v)|
            s.replace(&format!("${{{}}}", k), v))
    }

    /// Convert to environment variables for subprocess
    pub fn to_env_vars(&self) -> HashMap<String, String> {
        self.vars.iter().map(|(k, v)| {
            let env_key = format!("PAWL_{}", k.to_ascii_uppercase());
            (env_key, v.clone())
        }).collect()
    }

    /// Extend with extra key-value pairs
    pub fn extend(&mut self, extra: impl IntoIterator<Item = (String, String)>) {
        for (k, v) in extra {
            // Leak the key to get a &'static str — safe because these are small, finite strings
            let key: &'static str = Box::leak(k.into_boxed_str());
            self.vars.push((key, v));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("branch", "pawl/auth")
            .var("worktree", "/home/user/project/.pawl/worktrees/auth")
            .var("session", "my-project")
            .var("repo_root", "/home/user/project")
            .var("step", "Type check")
            .var("base_branch", "main")
            .var("claude_command", "claude");

        assert_eq!(ctx.expand("${task}"), "auth");
        assert_eq!(ctx.expand("${branch}"), "pawl/auth");
        assert_eq!(ctx.expand("${base_branch}"), "main");
        assert_eq!(
            ctx.expand("${worktree}"),
            "/home/user/project/.pawl/worktrees/auth"
        );
        assert_eq!(
            ctx.expand("git checkout ${branch}"),
            "git checkout pawl/auth"
        );
        assert_eq!(
            ctx.expand("git branch ${branch} ${base_branch}"),
            "git branch pawl/auth main"
        );
    }

    #[test]
    fn test_expand_full_context() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("branch", "pawl/auth")
            .var("worktree", "/home/user/project/.pawl/worktrees/auth")
            .var("session", "my-project")
            .var("repo_root", "/home/user/project")
            .var("step", "Develop")
            .var("base_branch", "develop")
            .var("claude_command", "claude")
            .var("step_index", "1")
            .var("log_file", "/home/user/project/.pawl/logs/auth.jsonl")
            .var("task_file", "/home/user/project/.pawl/tasks/auth.md");

        assert_eq!(
            ctx.expand("${log_file}"),
            "/home/user/project/.pawl/logs/auth.jsonl"
        );
        assert_eq!(
            ctx.expand("${task_file}"),
            "/home/user/project/.pawl/tasks/auth.md"
        );
        assert_eq!(ctx.expand("${step_index}"), "1");
        assert_eq!(ctx.expand("${base_branch}"), "develop");
        assert_eq!(
            ctx.expand("cat ${log_file}"),
            "cat /home/user/project/.pawl/logs/auth.jsonl"
        );
    }

    #[test]
    fn test_env_vars_full() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("branch", "pawl/auth")
            .var("worktree", "/home/user/project/.pawl/worktrees/auth")
            .var("session", "my-project")
            .var("repo_root", "/home/user/project")
            .var("step", "Develop")
            .var("base_branch", "main")
            .var("claude_command", "ccc")
            .var("step_index", "1")
            .var("log_file", "/logs/auth.jsonl")
            .var("task_file", "/tasks/auth.md");

        let env = ctx.to_env_vars();
        assert_eq!(env.get("PAWL_LOG_FILE"), Some(&"/logs/auth.jsonl".to_string()));
        assert_eq!(env.get("PAWL_TASK_FILE"), Some(&"/tasks/auth.md".to_string()));
        assert_eq!(env.get("PAWL_STEP_INDEX"), Some(&"1".to_string()));
        assert_eq!(env.get("PAWL_BASE_BRANCH"), Some(&"main".to_string()));
        assert_eq!(env.get("PAWL_CLAUDE_COMMAND"), Some(&"ccc".to_string()));
    }

    #[test]
    fn test_env_vars_basic() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("branch", "pawl/auth")
            .var("worktree", "/home/user/project/.pawl/worktrees/auth")
            .var("session", "my-project")
            .var("repo_root", "/home/user/project")
            .var("step", "Setup")
            .var("base_branch", "main")
            .var("claude_command", "claude");

        let env = ctx.to_env_vars();
        assert_eq!(env.get("PAWL_TASK"), Some(&"auth".to_string()));
        assert_eq!(env.get("PAWL_BRANCH"), Some(&"pawl/auth".to_string()));
        assert_eq!(env.get("PAWL_BASE_BRANCH"), Some(&"main".to_string()));
        assert!(env.get("PAWL_LOG_FILE").is_none());
        assert!(env.get("PAWL_TASK_FILE").is_none());
    }

    #[test]
    fn test_get() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("worktree", "/repo/.pawl/worktrees/auth");
        assert_eq!(ctx.get("task"), Some("auth"));
        assert_eq!(ctx.get("worktree"), Some("/repo/.pawl/worktrees/auth"));
        assert_eq!(ctx.get("nonexistent"), None);
    }
}
