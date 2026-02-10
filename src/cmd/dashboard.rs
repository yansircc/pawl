use anyhow::Result;
use serde::Serialize;
use tiny_http::{Header, Response, Server};

use super::common::Project;
use super::status::{build_task_detail, TaskDetail};
use crate::model::event::Event;

const DASHBOARD_HTML: &str = include_str!("templates/dashboard.html");

#[derive(Serialize)]
struct StatusResponse {
    project_root: String,
    workflow_steps: Vec<String>,
    tasks: Vec<TaskEntry>,
}

#[derive(Serialize)]
struct TaskEntry {
    #[serde(flatten)]
    detail: TaskDetail,
    blocked_by: Vec<String>,
    max_retries: usize,
}

#[derive(Serialize)]
struct EventEntry {
    ts: String,
    ts_ms: i64,
    task: String,
    #[serde(rename = "type")]
    event_type: String,
    step_name: Option<String>,
    detail: String,
}

#[derive(Serialize)]
struct EventsResponse {
    events: Vec<EventEntry>,
}

pub fn run(port: u16) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to start server: {}", e))?;

    eprintln!("pawl dashboard: http://localhost:{}", port);
    eprintln!("Press Ctrl+C to stop");

    loop {
        let request = match server.recv() {
            Ok(r) => r,
            Err(_) => continue,
        };

        let url = request.url().to_string();
        let response = match url.as_str() {
            "/" => serve_html(),
            "/api/status" => serve_status(),
            u if u.starts_with("/api/events") => serve_events(u),
            _ => not_found(),
        };

        let _ = request.respond(response);
    }
}

fn serve_html() -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(DASHBOARD_HTML)
        .with_header(content_type("text/html; charset=utf-8"))
}

fn serve_status() -> Response<std::io::Cursor<Vec<u8>>> {
    match build_status() {
        Ok(json) => json_response(&json),
        Err(e) => error_response(&e.to_string()),
    }
}

fn build_status() -> Result<String> {
    let project = Project::load()?;
    let tasks = project.discover_tasks()?;
    let workflow_steps: Vec<String> = project
        .config
        .workflow
        .iter()
        .map(|s| s.name.clone())
        .collect();

    let mut entries = Vec::new();
    for name in &tasks {
        let detail = build_task_detail(&project, name)?;
        let blocked_by = project.check_dependencies(name)?;
        let max_retries = if detail.current_step < project.config.workflow.len() {
            project.config.workflow[detail.current_step].effective_max_retries()
        } else {
            0
        };
        entries.push(TaskEntry {
            detail,
            blocked_by,
            max_retries,
        });
    }

    let resp = StatusResponse {
        project_root: project.project_root,
        workflow_steps,
        tasks: entries,
    };

    Ok(serde_json::to_string(&resp)?)
}

fn serve_events(url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match build_events(url) {
        Ok(json) => json_response(&json),
        Err(e) => error_response(&e.to_string()),
    }
}

fn build_events(url: &str) -> Result<String> {
    let since_ms = url
        .split("since=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .and_then(|s| s.parse::<i64>().ok());
    let since = since_ms.and_then(chrono::DateTime::from_timestamp_millis);

    let project = Project::load()?;
    let tasks = project.discover_tasks()?;

    let mut all_events: Vec<EventEntry> = Vec::new();

    for task_name in &tasks {
        let events = project.read_events(task_name)?;
        for event in &events {
            let ts = event.ts();

            if let Some(since_ts) = since {
                if ts <= since_ts {
                    continue;
                }
            }

            let step_name = event.step_index().map(|i| {
                project
                    .config
                    .workflow
                    .get(i)
                    .map(|s| s.name.clone())
                    .unwrap_or_else(|| format!("step-{}", i))
            });

            let detail = build_event_detail(event);

            all_events.push(EventEntry {
                ts: ts.to_rfc3339(),
                ts_ms: ts.timestamp_millis(),
                task: task_name.clone(),
                event_type: event.type_name().to_string(),
                step_name,
                detail,
            });
        }
    }

    all_events.sort_by(|a, b| b.ts_ms.cmp(&a.ts_ms));
    all_events.truncate(200);

    let resp = EventsResponse {
        events: all_events,
    };
    Ok(serde_json::to_string(&resp)?)
}

fn build_event_detail(event: &Event) -> String {
    match event {
        Event::StepFinished { success, .. } => {
            if *success {
                "ok".to_string()
            } else {
                "fail".to_string()
            }
        }
        Event::StepYielded { reason, .. } => reason.clone(),
        Event::StepResumed { .. } => String::new(),
        Event::StepReset { auto, .. } => {
            if *auto {
                "auto".to_string()
            } else {
                "manual".to_string()
            }
        }
        _ => String::new(),
    }
}

fn content_type(ct: &str) -> Header {
    Header::from_bytes("Content-Type", ct).unwrap()
}

fn json_response(json: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(json).with_header(content_type("application/json"))
}

fn error_response(msg: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    let json = serde_json::json!({"error": msg}).to_string();
    Response::from_string(json)
        .with_status_code(500)
        .with_header(content_type("application/json"))
}

fn not_found() -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string("404").with_status_code(404)
}
