pub mod agent;
pub mod capture;
pub mod common;
pub mod control;
pub mod create;
pub mod enter;
pub mod init;
pub mod log;
pub mod start;
pub mod status;
pub mod wait;

use crate::cli::Command;
use anyhow::Result;

pub fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Init => init::run(),
        Command::Create { name, description, depends } => {
            create::run(&name, description.as_deref(), depends.as_deref())
        }
        Command::List => status::list(false),
        Command::Start { task } => start::run(&task),
        Command::Status { task, json } => status::run(task.as_deref(), json),
        Command::Next { task } => control::next(&task),
        Command::Retry { task } => control::retry(&task),
        Command::Back { task } => control::back(&task),
        Command::Skip { task } => control::skip(&task),
        Command::Stop { task } => control::stop(&task),
        Command::Reset { task } => control::reset(&task),
        Command::Enter { task } => enter::run(&task),
        Command::Capture { task, lines, json } => capture::run(&task, lines, json),
        Command::Wait { task, until, timeout, interval } => {
            wait::run(&task, &until, timeout, interval)
        }
        Command::Log { task, step, all } => log::run(&task, step, all),
        Command::Done { task, message } => agent::done(&task, message.as_deref()),
        Command::Fail { task, message } => agent::fail(&task, message.as_deref()),
        Command::Block { task, message } => agent::block(&task, message.as_deref()),
        Command::OnExit { task, exit_code } => control::on_exit(&task, exit_code),
    }
}
