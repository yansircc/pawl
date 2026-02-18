use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use tiny_http::{Header, Method, Response, Server};

use super::common::Project;
use super::status::{build_task_detail, TaskDetail};
use crate::model::event::Event;

#[derive(Serialize)]
struct WorkflowInfo {
    steps: Vec<String>,
    hooks: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct StatusResponse {
    project_root: String,
    workflows: std::collections::HashMap<String, WorkflowInfo>,
    tasks: Vec<TaskEntry>,
}

#[derive(Serialize)]
struct TaskEntry {
    #[serde(flatten)]
    detail: TaskDetail,
    workflow: String,
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

pub fn run(port: u16, ui_path: Option<&str>) -> Result<()> {
    let (ui_dir, ui_index) = match ui_path {
        Some(path) => {
            let p = Path::new(path);
            if !p.exists() {
                anyhow::bail!("UI file not found: {}", path);
            }
            let dir = p
                .parent()
                .unwrap_or(Path::new("."))
                .canonicalize()
                .map_err(|e| anyhow::anyhow!("Failed to resolve UI directory: {}", e))?;
            let index = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            (Some(dir), index)
        }
        None => (None, String::new()),
    };

    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr)
        .map_err(|e| anyhow::anyhow!("Failed to start server: {}", e))?;

    eprintln!("pawl serve: http://localhost:{}", port);

    loop {
        let request = match server.recv() {
            Ok(r) => r,
            Err(_) => break Ok(()),
        };

        if *request.method() == Method::Options {
            let _ = request.respond(preflight_response());
            continue;
        }

        let url = request.url().to_string();
        let response = match url.as_str() {
            "/" => serve_root(&ui_dir, &ui_index),
            "/api/status" => with_cors(serve_status()),
            u if u.starts_with("/api/stream/") => with_cors(serve_stream(u)),
            u if u.starts_with("/api/events") => with_cors(serve_events(u)),
            u if !u.starts_with("/api/") => serve_static(&ui_dir, u),
            _ => not_found(),
        };

        let _ = request.respond(response);
    }
}

fn serve_root(
    ui_dir: &Option<PathBuf>,
    ui_index: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    match ui_dir {
        Some(dir) => {
            let file_path = dir.join(ui_index);
            match std::fs::read_to_string(&file_path) {
                Ok(html) => Response::from_string(html)
                    .with_header(content_type("text/html; charset=utf-8")),
                Err(_) => not_found(),
            }
        }
        None => {
            let discovery = serde_json::json!({
                "endpoints": [
                    {"path": "/api/status", "description": "Task status and workflow info"},
                    {"path": "/api/events?since=<ms>", "description": "Event stream (newest first, max 200)"},
                    {"path": "/api/stream/<task>?offset=<bytes>", "description": "Streaming stdout for running task"},
                ]
            });
            json_response(&discovery.to_string())
        }
    }
}

fn serve_static(
    ui_dir: &Option<PathBuf>,
    url_path: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let dir = match ui_dir {
        Some(d) => d,
        None => return not_found(),
    };

    // Strip leading slash
    let rel = url_path.trim_start_matches('/');
    if rel.is_empty() {
        return not_found();
    }

    // Reject path traversal
    for component in Path::new(rel).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return not_found();
        }
    }

    let file_path = dir.join(rel);

    // Verify resolved path is within ui_dir
    match file_path.canonicalize() {
        Ok(resolved) => {
            if !resolved.starts_with(dir) {
                return not_found();
            }
        }
        Err(_) => return not_found(),
    }

    match std::fs::read(&file_path) {
        Ok(data) => {
            let ct = match file_path
                .extension()
                .and_then(|e| e.to_str())
            {
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("html") | Some("htm") => "text/html; charset=utf-8",
                Some("json") => "application/json",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                Some("ico") => "image/x-icon",
                _ => "application/octet-stream",
            };
            Response::from_data(data).with_header(content_type(ct))
        }
        Err(_) => not_found(),
    }
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

    let mut workflows_map = std::collections::HashMap::new();
    for (wf_name, config) in project.all_workflows() {
        let steps = config.workflow.iter().map(|s| s.name.clone()).collect();
        let hooks = config.on.clone();
        workflows_map.insert(wf_name.clone(), WorkflowInfo { steps, hooks });
    }

    let mut entries = Vec::new();
    for name in &tasks {
        let detail = build_task_detail(&project, name)?;
        let blocked_by = project.check_dependencies(name)?;
        let (wf_name, config) = project.workflow_for(name)?;
        let wf_name = wf_name.to_string();
        let max_retries = if detail.current_step < config.workflow.len() {
            config.workflow[detail.current_step].effective_max_retries()
        } else {
            0
        };
        entries.push(TaskEntry {
            detail,
            workflow: wf_name,
            blocked_by,
            max_retries,
        });
    }

    let resp = StatusResponse {
        project_root: project.project_root,
        workflows: workflows_map,
        tasks: entries,
    };

    Ok(serde_json::to_string(&resp)?)
}

fn serve_stream(url: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    match build_stream(url) {
        Ok(json) => json_response(&json),
        Err(e) => error_response(&e.to_string()),
    }
}

fn build_stream(url: &str) -> Result<String> {
    // Parse: /api/stream/{task}?offset=N
    let path = url.split('?').next().unwrap_or(url);
    let task_name = path.strip_prefix("/api/stream/").unwrap_or("");
    if task_name.is_empty() {
        anyhow::bail!("Missing task name");
    }

    let offset: u64 = url
        .split("offset=")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let project = Project::load()?;
    let stream_file = project.stream_file(task_name);

    if !stream_file.exists() {
        let resp = serde_json::json!({
            "task": task_name,
            "content": "",
            "offset": 0,
            "active": false,
        });
        return Ok(resp.to_string());
    }

    let data = std::fs::read(&stream_file).unwrap_or_default();
    let file_len = data.len() as u64;
    let start = (offset as usize).min(data.len());
    let content = String::from_utf8_lossy(&data[start..]).to_string();

    let resp = serde_json::json!({
        "task": task_name,
        "content": content,
        "offset": file_len,
        "active": true,
    });
    Ok(resp.to_string())
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
                project.step_name(task_name, i).to_string()
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

fn cors_header() -> Header {
    Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap()
}

fn with_cors(
    response: Response<std::io::Cursor<Vec<u8>>>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    response.with_header(cors_header())
}

fn preflight_response() -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string("")
        .with_status_code(204)
        .with_header(cors_header())
        .with_header(Header::from_bytes("Access-Control-Allow-Methods", "GET, OPTIONS").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Headers", "Content-Type").unwrap())
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
