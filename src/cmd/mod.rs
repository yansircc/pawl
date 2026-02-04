pub mod agent;
pub mod common;
pub mod control;
pub mod create;
pub mod enter;
pub mod init;
pub mod log;
pub mod start;
pub mod status;

use crate::cli::Command;
use anyhow::Result;

pub fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Init => init::run(),
        Command::Create { name, description, depends } => {
            create::run(&name, description.as_deref(), depends.as_deref())
        }
        Command::List => status::list(),
        Command::Start { task } => start::run(&task),
        Command::Status { task } => status::run(task.as_deref()),
        Command::Next { task } => control::next(&task),
        Command::Retry { task } => control::retry(&task),
        Command::Back { task } => control::back(&task),
        Command::Skip { task } => control::skip(&task),
        Command::Stop { task } => control::stop(&task),
        Command::Reset { task } => control::reset(&task),
        Command::Enter { task } => enter::run(&task),
        Command::Log { task } => log::run(&task),
        Command::Done { task, message } => agent::done(&task, message.as_deref()),
        Command::Fail { task, message } => agent::fail(&task, message.as_deref()),
        Command::Block { task, message } => agent::block(&task, message.as_deref()),
        Command::OnExit { task } => control::on_exit(&task),
    }
}
