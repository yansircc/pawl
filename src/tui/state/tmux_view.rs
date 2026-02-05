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
