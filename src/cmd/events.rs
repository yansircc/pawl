use anyhow::Result;
use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::mpsc;

use super::common::Project;

/// Unified event stream: output JSONL events from all (or one) task log files.
/// With --follow, watches for new events in real-time.
pub fn run(task_filter: Option<&str>, follow: bool) -> Result<()> {
    let project = Project::load()?;
    let logs_dir = project.pawl_dir.join("logs");

    if !logs_dir.exists() {
        if follow {
            // Create dir so watcher has something to watch
            std::fs::create_dir_all(&logs_dir)?;
        } else {
            return Ok(());
        }
    }

    // Resolve task filter
    let task_filter = task_filter
        .map(|t| project.resolve_task_name(t))
        .transpose()?;

    // Discover existing log files
    let log_files = discover_log_files(&logs_dir, task_filter.as_deref())?;

    // Print existing events (sorted by timestamp across files)
    let mut file_offsets: HashMap<PathBuf, u64> = HashMap::new();
    for (task_name, path) in &log_files {
        let offset = print_events_from_file(task_name, path, 0)?;
        file_offsets.insert(path.clone(), offset);
    }

    if !follow {
        return Ok(());
    }

    // Watch for changes
    let (tx, rx) = mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default(),
    )?;

    watcher.watch(&logs_dir, RecursiveMode::NonRecursive)?;

    // Poll for events
    loop {
        match rx.recv() {
            Ok(event) => {
                if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    continue;
                }

                for path in &event.paths {
                    let Some(ext) = path.extension() else {
                        continue;
                    };
                    if ext != "jsonl" {
                        continue;
                    }

                    let task_name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();

                    // Apply task filter
                    if let Some(ref filter) = task_filter {
                        if task_name != *filter {
                            continue;
                        }
                    }

                    let offset = file_offsets.get(path).copied().unwrap_or(0);
                    match print_events_from_file(&task_name, path, offset) {
                        Ok(new_offset) => {
                            file_offsets.insert(path.clone(), new_offset);
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}

/// Discover JSONL log files, optionally filtered by task name.
fn discover_log_files(
    logs_dir: &std::path::Path,
    task_filter: Option<&str>,
) -> Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();

    if !logs_dir.exists() {
        return Ok(files);
    }

    for entry in std::fs::read_dir(logs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
            continue;
        }

        let task_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if let Some(filter) = task_filter {
            if task_name != filter {
                continue;
            }
        }

        files.push((task_name, path));
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// Print new JSONL events from a file starting at the given byte offset.
/// Each line is prefixed with a "task" field injected into the JSON.
/// Returns the new byte offset after reading.
fn print_events_from_file(task_name: &str, path: &std::path::Path, offset: u64) -> Result<u64> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    let file_len = metadata.len();

    if file_len <= offset {
        return Ok(offset);
    }

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(offset))?;

    let mut current_pos = offset;

    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            break;
        }
        current_pos += bytes_read as u64;

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Inject "task" field into the JSON object
        if line.starts_with('{') {
            // Insert task name as the first field
            println!("{{\"task\":\"{}\",{}", task_name, &line[1..]);
        } else {
            println!("{}", line);
        }
    }

    Ok(current_pos)
}
