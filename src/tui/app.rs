use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use super::data::{DataProvider, LiveDataProvider, TaskAction};
use super::event::{handle_key_event, Action};
use super::state::{reducer::reduce, AppState, ModalState, StatusMessage, ViewMode};
use super::view;

const TICK_RATE: Duration = Duration::from_millis(250);
const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const TMUX_CAPTURE_LINES: usize = 100;

pub fn run() -> Result<()> {
    let provider = LiveDataProvider::new()?;
    run_with_provider(Box::new(provider))
}

pub fn run_with_provider(provider: Box<dyn DataProvider>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal, provider);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    provider: Box<dyn DataProvider>,
) -> Result<()> {
    let mut state = AppState::new();
    let mut last_refresh = Instant::now();

    // Initial load
    state = refresh_data(state, &provider);

    loop {
        // Render
        terminal.draw(|f| view::render(f, &state))?;

        // Handle events with timeout
        let timeout = TICK_RATE;
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events, not release
                if key.kind == KeyEventKind::Press {
                    if let Some(action) = handle_key_event(key, &state) {
                        state = handle_action(state, action, &provider);
                    }
                }
            }
        }

        // Check for quit
        if state.should_quit {
            break;
        }

        // Periodic refresh
        if last_refresh.elapsed() >= REFRESH_INTERVAL {
            state = refresh_data(state, &provider);
            last_refresh = Instant::now();
        }

        // Clear expired status messages
        state = state.clear_expired_status();

        // Refresh tmux content if in tmux view
        if let ViewMode::TmuxView(task_name) = state.view.clone() {
            state = refresh_tmux(state, &task_name, &provider);
        }
    }

    Ok(())
}

fn handle_action(
    mut state: AppState,
    action: Action,
    provider: &Box<dyn DataProvider>,
) -> AppState {
    // Handle ConfirmYes specially - extract the stored action before reducing
    let confirmed_action = if matches!(action, Action::ConfirmYes) {
        if let Some(ModalState::Confirm { on_confirm, .. }) = &state.modal {
            Some(on_confirm.as_ref().clone())
        } else {
            None
        }
    } else {
        None
    };

    // Apply reducer for navigation/view changes
    state = reduce(state, action.clone());

    // If this was a confirmation, recursively handle the confirmed action
    if let Some(confirmed) = confirmed_action {
        return handle_action(state, confirmed, provider);
    }

    // Then handle side effects
    match action {
        Action::Refresh => {
            state = refresh_data(state, provider);
        }

        Action::Enter => {
            // Load detail when entering detail view
            if let ViewMode::TaskDetail(name) = state.view.clone() {
                state = load_task_detail(state, &name, provider);
            }
        }

        Action::SwitchToTmux(ref name) => {
            state = refresh_tmux(state, name, provider);
        }

        Action::StartTask(name) => {
            state = execute_task_action(state, TaskAction::Start(name), provider);
        }
        Action::StopTask(name) => {
            state = execute_task_action(state, TaskAction::Stop(name), provider);
        }
        Action::ResetTask(name) => {
            state = execute_task_action(state, TaskAction::Reset(name), provider);
        }
        Action::NextTask(name) => {
            state = execute_task_action(state, TaskAction::Next(name), provider);
        }
        Action::RetryTask(name) => {
            state = execute_task_action(state, TaskAction::Retry(name), provider);
        }
        Action::SkipTask(name) => {
            state = execute_task_action(state, TaskAction::Skip(name), provider);
        }
        Action::DoneTask(name) => {
            state = execute_task_action(state, TaskAction::Done(name), provider);
        }
        Action::FailTask(name) => {
            state = execute_task_action(state, TaskAction::Fail(name), provider);
        }
        Action::BlockTask(name) => {
            state = execute_task_action(state, TaskAction::Block(name), provider);
        }

        _ => {}
    }

    state
}

fn refresh_data(mut state: AppState, provider: &Box<dyn DataProvider>) -> AppState {
    match provider.load_tasks() {
        Ok(tasks) => {
            state.task_list = state.task_list.update_tasks(tasks);
        }
        Err(e) => {
            state = state.set_status(StatusMessage::error(format!("Load error: {}", e)));
        }
    }

    // Also refresh detail if viewing one
    if let ViewMode::TaskDetail(name) = state.view.clone() {
        state = load_task_detail(state, &name, provider);
    }

    state
}

fn load_task_detail(
    mut state: AppState,
    name: &str,
    provider: &Box<dyn DataProvider>,
) -> AppState {
    match provider.load_task_detail(name) {
        Ok(detail) => {
            state.task_detail = Some(detail);
        }
        Err(e) => {
            state = state.set_status(StatusMessage::error(format!("Load error: {}", e)));
        }
    }
    state
}

fn refresh_tmux(mut state: AppState, task_name: &str, provider: &Box<dyn DataProvider>) -> AppState {
    match provider.capture_tmux(task_name, TMUX_CAPTURE_LINES) {
        Ok(result) => {
            if let Some(ref mut tmux) = state.tmux_view {
                *tmux = tmux.update_content(result.content, result.window_exists);
            }
        }
        Err(_) => {
            // Silently ignore capture errors
        }
    }
    state
}

fn execute_task_action(
    mut state: AppState,
    action: TaskAction,
    provider: &Box<dyn DataProvider>,
) -> AppState {
    let action_name = match &action {
        TaskAction::Start(_) => "Started",
        TaskAction::Stop(_) => "Stopped",
        TaskAction::Reset(_) => "Reset",
        TaskAction::Next(_) => "Advanced",
        TaskAction::Retry(_) => "Retrying",
        TaskAction::Skip(_) => "Skipped",
        TaskAction::Done(_) => "Marked done",
        TaskAction::Fail(_) => "Marked failed",
        TaskAction::Block(_) => "Marked blocked",
    };

    match provider.execute_action(&action) {
        Ok(()) => {
            state = state.set_status(StatusMessage::info(action_name));
            // Refresh data after action
            state = refresh_data(state, provider);
        }
        Err(e) => {
            state = state.set_status(StatusMessage::error(format!("{}: {}", action_name, e)));
        }
    }

    state
}
