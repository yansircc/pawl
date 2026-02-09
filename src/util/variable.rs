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

    /// Add a variable with an owned key (for user-defined vars from config.vars)
    pub fn var_owned(mut self, name: String, value: String) -> Self {
        let key: &'static str = Box::leak(name.into_boxed_str());
        self.vars.push((key, value));
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
            .var("session", "my-project")
            .var("project_root", "/home/user/project")
            .var("step", "Type check");

        assert_eq!(ctx.expand("${task}"), "auth");
        assert_eq!(ctx.expand("${project_root}"), "/home/user/project");
        assert_eq!(
            ctx.expand("echo ${task} in ${project_root}"),
            "echo auth in /home/user/project"
        );
    }

    #[test]
    fn test_expand_with_user_vars() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("project_root", "/home/user/project")
            .var_owned("branch".to_string(), "pawl/auth".to_string())
            .var_owned("worktree".to_string(), "/home/user/project/.pawl/worktrees/auth".to_string());

        assert_eq!(ctx.expand("${branch}"), "pawl/auth");
        assert_eq!(
            ctx.expand("git checkout ${branch}"),
            "git checkout pawl/auth"
        );
        assert_eq!(
            ctx.expand("cd ${worktree} && npm test"),
            "cd /home/user/project/.pawl/worktrees/auth && npm test"
        );
    }

    #[test]
    fn test_expand_full_context() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("session", "my-project")
            .var("project_root", "/home/user/project")
            .var("step", "Develop")
            .var("step_index", "1")
            .var("log_file", "/home/user/project/.pawl/logs/auth.jsonl");

        assert_eq!(
            ctx.expand("${log_file}"),
            "/home/user/project/.pawl/logs/auth.jsonl"
        );
        assert_eq!(ctx.expand("${step_index}"), "1");
        assert_eq!(
            ctx.expand("cat ${log_file}"),
            "cat /home/user/project/.pawl/logs/auth.jsonl"
        );
    }

    #[test]
    fn test_env_vars() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("session", "my-project")
            .var("project_root", "/home/user/project")
            .var("step", "Setup")
            .var("step_index", "1")
            .var("log_file", "/logs/auth.jsonl");

        let env = ctx.to_env_vars();
        assert_eq!(env.get("PAWL_TASK"), Some(&"auth".to_string()));
        assert_eq!(env.get("PAWL_PROJECT_ROOT"), Some(&"/home/user/project".to_string()));
        assert_eq!(env.get("PAWL_LOG_FILE"), Some(&"/logs/auth.jsonl".to_string()));
        assert_eq!(env.get("PAWL_STEP_INDEX"), Some(&"1".to_string()));
    }

    #[test]
    fn test_env_vars_with_user_vars() {
        let ctx = Context::build()
            .var("task", "auth")
            .var_owned("branch".to_string(), "pawl/auth".to_string())
            .var_owned("base_branch".to_string(), "main".to_string());

        let env = ctx.to_env_vars();
        assert_eq!(env.get("PAWL_TASK"), Some(&"auth".to_string()));
        assert_eq!(env.get("PAWL_BRANCH"), Some(&"pawl/auth".to_string()));
        assert_eq!(env.get("PAWL_BASE_BRANCH"), Some(&"main".to_string()));
    }

    #[test]
    fn test_get() {
        let ctx = Context::build()
            .var("task", "auth")
            .var("project_root", "/repo");
        assert_eq!(ctx.get("task"), Some("auth"));
        assert_eq!(ctx.get("project_root"), Some("/repo"));
        assert_eq!(ctx.get("nonexistent"), None);
    }

    #[test]
    fn test_user_var_expansion_order() {
        // User vars can reference intrinsic vars and earlier user vars
        let mut ctx = Context::build()
            .var("task", "auth")
            .var("project_root", "/repo");

        // Simulate config.vars expansion
        let branch_template = "pawl/${task}";
        let expanded_branch = ctx.expand(branch_template);
        ctx = ctx.var_owned("branch".to_string(), expanded_branch);

        let worktree_template = "${project_root}/.pawl/worktrees/${task}";
        let expanded_worktree = ctx.expand(worktree_template);
        ctx = ctx.var_owned("worktree".to_string(), expanded_worktree);

        assert_eq!(ctx.get("branch"), Some("pawl/auth"));
        assert_eq!(ctx.get("worktree"), Some("/repo/.pawl/worktrees/auth"));
    }
}
