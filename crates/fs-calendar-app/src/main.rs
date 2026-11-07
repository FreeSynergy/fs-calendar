//! The binary: wiring only.
//!
//! Pattern: none (thin binary) — parse arguments, connect to the servers, import, then export or show.
//! The runtime lives as long as the window: the workbench's calls run on it.

use std::sync::Arc;

use fs_calendar_app::args::{Args, HELP, parse};
use fs_calendar_app::{
    CalendarAdapter, cannot_reach, export_all, import_file, labels, no_socket, no_tasks_socket,
    not_exported, screen_widget, shown_zone, tasks_cannot_reach, wrote,
};
use fs_calendar_client::{RpcCalendar, RpcTaskLines, TaskLines};
use fs_components_app::{Status, WindowOptions, launch};
use fs_components_workbenches::calendar::{CalendarWidget, Moment};

fn main() {
    if let Err(message) = run() {
        eprintln!("fs-calendar-app: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = parse(&argv)?;
    if args.help {
        println!("{HELP}");
        return Ok(());
    }
    let socket = args.socket.as_deref().ok_or_else(no_socket)?;
    let (zone, zone_note) = shown_zone(args.zone.as_deref())?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let calendar = Arc::new(
        RpcCalendar::connect(socket)
            .map_err(|e| cannot_reach(&socket.display().to_string(), &e.to_string()))?,
    );
    let tasks: Option<Arc<dyn TaskLines>> = match &args.tasks_socket {
        Some(path) => Some(Arc::new(RpcTaskLines::connect(path).map_err(|e| {
            tasks_cannot_reach(&path.display().to_string(), &e.to_string())
        })?)),
        None => None,
    };
    let handle = runtime.handle().clone();

    let mut status = None;
    if let Some(file) = &args.import {
        let summary = import_file(calendar.as_ref(), &handle, file)?;
        eprintln!("{}", summary.message());
        status = Some(if summary.is_clean() {
            Status::Info(summary.message())
        } else {
            Status::Problem(summary.message())
        });
    }
    if let Some(note) = zone_note {
        eprintln!("{note}");
        status = Some(Status::Problem(match status {
            Some(Status::Info(text) | Status::Problem(text)) => format!("{note}. {text}"),
            _ => note,
        }));
    }
    if tasks.is_none() && args.export.is_none() {
        let note = no_tasks_socket();
        eprintln!("{note}");
        status = Some(match status {
            Some(Status::Info(text)) => Status::Info(format!("{text}. {note}")),
            Some(Status::Problem(text)) => Status::Problem(format!("{text}. {note}")),
            Some(other) => other,
            None => Status::Info(note),
        });
    }
    if let Some(file) = &args.export {
        let report = export_all(calendar.as_ref(), &handle, file, &args.export_sensitive)?;
        eprintln!("{}", report.message(file));
        if !report.failed.is_empty() {
            return Err(not_exported(report.failed.len()));
        }
        if args.snapshot.is_none() {
            return Ok(());
        }
    }
    show(&args, calendar, tasks, handle, zone, status)
}

fn show(
    args: &Args,
    calendar: Arc<RpcCalendar>,
    tasks: Option<Arc<dyn TaskLines>>,
    handle: tokio::runtime::Handle,
    zone: fs_calendar_core::Zone,
    status: Option<Status>,
) -> Result<(), String> {
    let mut adapter =
        CalendarAdapter::new(calendar.clone(), calendar, handle, zone).week_start(args.week_start);
    if let Some(tasks) = tasks {
        adapter = adapter.with_tasks(tasks);
    }
    if let Some(day) = args.date {
        adapter = adapter.frozen_at(Moment::at(day, 12, 0));
    }
    let widget = CalendarWidget::new(Arc::new(adapter), labels());
    if let Some(view) = args.view {
        widget.with_state(|s| s.set_view(view));
    }
    let app = screen_widget(widget).map_err(|e| e.to_string())?;
    app.set_status(status);
    let (width, height) = args.size;
    if let Some(file) = &args.snapshot {
        app.picture(width, height, file)
            .map_err(|e| e.to_string())?;
        eprintln!("{}", wrote(&file.display().to_string()));
        return Ok(());
    }
    let mut options = WindowOptions::default();
    options.width = width;
    options.height = height;
    launch(options, app).map_err(|e| e.to_string())
}
