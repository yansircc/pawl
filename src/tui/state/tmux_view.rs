/// State for the tmux pane view
#[derive(Debug, Clone)]
pub struct TmuxViewState {
    pub task_name: String,
    pub content: String,
    pub scroll_offset: usize,
    pub auto_scroll: bool,
    pub window_exists: bool,
}

impl TmuxViewState {
    pub fn new(task_name: String) -> Self {
        Self {
            task_name,
            content: String::new(),
            scroll_offset: 0,
            auto_scroll: true,
            window_exists: false,
        }
    }

    pub fn update_content(&self, content: String, window_exists: bool) -> Self {
        let lines: Vec<&str> = content.lines().collect();
        let scroll_offset = if self.auto_scroll {
            // Auto-scroll to bottom
            lines.len().saturating_sub(1)
        } else {
            self.scroll_offset
        };
        Self {
            content,
            scroll_offset,
            window_exists,
            ..self.clone()
        }
    }

    pub fn scroll_up(&self, lines: usize) -> Self {
        Self {
            scroll_offset: self.scroll_offset.saturating_sub(lines),
            auto_scroll: false,
            ..self.clone()
        }
    }

    pub fn scroll_down(&self, lines: usize) -> Self {
        let content_lines: Vec<&str> = self.content.lines().collect();
        let max_offset = content_lines.len().saturating_sub(1);
        let new_offset = (self.scroll_offset + lines).min(max_offset);
        // Re-enable auto-scroll if at bottom
        let auto_scroll = new_offset >= max_offset;
        Self {
            scroll_offset: new_offset,
            auto_scroll,
            ..self.clone()
        }
    }

    pub fn content_lines(&self) -> Vec<&str> {
        self.content.lines().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let state = TmuxViewState::new("test-task".to_string());
        assert_eq!(state.task_name, "test-task");
        assert!(state.content.is_empty());
        assert_eq!(state.scroll_offset, 0);
        assert!(state.auto_scroll);
        assert!(!state.window_exists);
    }

    #[test]
    fn test_update_content_auto_scroll() {
        let state = TmuxViewState::new("task".to_string());
        assert!(state.auto_scroll);

        let content = "line1\nline2\nline3\nline4\nline5".to_string();
        let state = state.update_content(content, true);

        assert!(state.window_exists);
        assert_eq!(state.scroll_offset, 4); // auto-scroll to bottom (5 lines - 1)
        assert!(state.auto_scroll);
    }

    #[test]
    fn test_scroll_up_disables_auto_scroll() {
        let state = TmuxViewState::new("task".to_string());
        let content = "line1\nline2\nline3\nline4\nline5".to_string();
        let state = state.update_content(content, true);

        assert!(state.auto_scroll);
        assert_eq!(state.scroll_offset, 4);

        let state = state.scroll_up(2);
        assert!(!state.auto_scroll);
        assert_eq!(state.scroll_offset, 2);

        // Subsequent update should not auto-scroll
        let new_content = "line1\nline2\nline3\nline4\nline5\nline6".to_string();
        let state = state.update_content(new_content, true);
        assert_eq!(state.scroll_offset, 2); // preserved
        assert!(!state.auto_scroll);
    }

    #[test]
    fn test_scroll_down_re_enables_auto_scroll() {
        let mut state = TmuxViewState::new("task".to_string());
        state.content = "line1\nline2\nline3\nline4\nline5".to_string();
        state.scroll_offset = 0;
        state.auto_scroll = false;

        // Scroll down but not to bottom
        let state = state.scroll_down(2);
        assert_eq!(state.scroll_offset, 2);
        assert!(!state.auto_scroll);

        // Scroll to bottom
        let state = state.scroll_down(10);
        assert_eq!(state.scroll_offset, 4); // max offset
        assert!(state.auto_scroll); // re-enabled
    }

    #[test]
    fn test_scroll_up_boundary() {
        let mut state = TmuxViewState::new("task".to_string());
        state.scroll_offset = 2;

        let state = state.scroll_up(5); // try to go below 0
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_content_lines() {
        let mut state = TmuxViewState::new("task".to_string());
        state.content = "line1\nline2\nline3".to_string();

        let lines = state.content_lines();
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_empty_content() {
        let state = TmuxViewState::new("task".to_string());
        let lines = state.content_lines();
        assert!(lines.is_empty());

        let state = state.update_content(String::new(), false);
        assert_eq!(state.scroll_offset, 0);
    }
}
