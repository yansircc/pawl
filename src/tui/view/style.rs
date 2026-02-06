use ratatui::style::{Color, Modifier, Style};

use crate::model::TaskStatus;
use crate::tui::state::task_detail::StepItemStatus;

/// Color scheme for the TUI
pub struct Theme;

impl Theme {
    // Status colors
    pub fn status_color(status: TaskStatus) -> Color {
        match status {
            TaskStatus::Pending => Color::Gray,
            TaskStatus::Running => Color::Blue,
            TaskStatus::Waiting => Color::Yellow,
            TaskStatus::Completed => Color::Green,
            TaskStatus::Failed => Color::Red,
            TaskStatus::Stopped => Color::Magenta,
        }
    }

    pub fn step_status_color(status: StepItemStatus) -> Color {
        match status {
            StepItemStatus::Pending => Color::Gray,
            StepItemStatus::Current => Color::Cyan,
            StepItemStatus::Success => Color::Green,
            StepItemStatus::Failed => Color::Red,
            StepItemStatus::Skipped => Color::DarkGray,
        }
    }

    // General styles
    pub fn title() -> Style {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    }

    pub fn selected() -> Style {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    }

    pub fn normal() -> Style {
        Style::default()
    }

    pub fn dimmed() -> Style {
        Style::default().fg(Color::DarkGray)
    }

    pub fn highlight() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn border() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn border_focused() -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn status_bar() -> Style {
        Style::default().bg(Color::DarkGray)
    }

    pub fn status_message() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn status_error() -> Style {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    }

    pub fn help_key() -> Style {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }

    pub fn help_desc() -> Style {
        Style::default().fg(Color::White)
    }
}

/// Format task status as styled text
pub fn format_status(status: TaskStatus) -> (&'static str, Style) {
    let text = match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Waiting => "waiting",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Stopped => "stopped",
    };
    (text, Style::default().fg(Theme::status_color(status)))
}

/// Status marker character
pub fn status_marker(status: StepItemStatus) -> &'static str {
    match status {
        StepItemStatus::Pending => " ",
        StepItemStatus::Current => ">",
        StepItemStatus::Success => "o",
        StepItemStatus::Failed => "x",
        StepItemStatus::Skipped => "-",
    }
}
