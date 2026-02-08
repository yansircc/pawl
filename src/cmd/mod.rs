pub mod capture;
pub mod common;
pub mod control;
pub mod create;
pub mod done;
pub mod enter;
pub mod events;
pub mod init;
pub mod log;
pub mod run;
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
        Command::List => status::list(),
        Command::Start { task, reset } => start::run(&task, reset),
        Command::Status { task } => status::run(task.as_deref()),
        Command::Stop { task } => control::stop(&task),
        Command::Reset { task, step } => control::reset(&task, step),
        Command::Enter { task } => enter::run(&task),
        Command::Capture { task, lines } => capture::run(&task, lines),
        Command::Wait { task, until, timeout, interval } => {
            wait::run(&task, &until, timeout, interval)
        }
        Command::Log { task, step, all } => log::run(&task, step, all),
        Command::Events { task, follow, event_type } => {
            events::run(task.as_deref(), follow, event_type.as_deref())
        }
        Command::Done { task, message } => done::done(&task, message.as_deref()),
        Command::Run { task, step } => run::run_in_viewport(&task, step),
    }
}
