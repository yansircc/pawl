pub mod approve;
pub mod capture;
pub mod common;
pub mod control;
pub mod create;
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
        Command::List => status::list(false),
        Command::Start { task, reset } => start::run(&task, reset),
        Command::Status { task, json } => status::run(task.as_deref(), json),
        Command::Stop { task } => control::stop(&task),
        Command::Reset { task, step } => control::reset(&task, step),
        Command::Enter { task } => enter::run(&task),
        Command::Capture { task, lines, json } => capture::run(&task, lines, json),
        Command::Wait { task, until, timeout, interval } => {
            wait::run(&task, &until, timeout, interval)
        }
        Command::Log { task, step, all, all_runs, jsonl } => log::run(&task, step, all, all_runs, jsonl),
        Command::Events { task, follow } => events::run(task.as_deref(), follow),
        Command::Done { task, message } => approve::done(&task, message.as_deref()),
        Command::Run { task, step } => run::run_in_viewport(&task, step),
    }
}
