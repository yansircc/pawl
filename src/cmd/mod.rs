pub mod common;
pub mod control;
pub mod serve;
pub mod done;
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
        Command::List => status::list(),
        Command::Start { task, reset } => start::run(&task, reset),
        Command::Status { task } => status::run(task.as_deref()),
        Command::Stop { task } => control::stop(&task),
        Command::Reset { task, step } => control::reset(&task, step),
        Command::Wait { tasks, until, timeout, interval, any } => {
            wait::run(&tasks, &until, timeout, interval, any)
        }
        Command::Log { task, step, all } => log::run(&task, step, all),
        Command::Events { task, follow, event_type } => {
            events::run(task.as_deref(), follow, event_type.as_deref())
        }
        Command::Done { task, message } => done::done(&task, message.as_deref()),
        Command::Serve { port, ui } => serve::run(port, ui.as_deref()),
        Command::Run { task, step } => run::run_in_viewport(&task, step),
    }
}
