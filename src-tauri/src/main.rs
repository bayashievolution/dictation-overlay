#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod native_messaging;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::{Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent};

use native_messaging::{read_message, send_message, InMessage, MonitorInfo, OutMessage};

const CAPABILITIES: &[&str] = &[
    "transparency",
    "always-on-top",
    "click-through",
    "multi-monitor",
    "position-report",
];

/// Bottom margin (px) when auto-positioning on a monitor.
const BOTTOM_MARGIN_PX: i32 = 80;

/// How long to wait between position_changed emits (debounce).
const POSITION_REPORT_INTERVAL_MS: u64 = 150;

/// Serializes concurrent stdout writes.
static STDOUT_LOCK: Mutex<()> = Mutex::new(());

/// Set when the window emits a Moved or Resized event.
/// The reporter thread clears it after sending one `position_changed`.
static GEOMETRY_DIRTY: AtomicBool = AtomicBool::new(false);

fn send(msg: &OutMessage) {
    let _g = STDOUT_LOCK.lock().ok();
    let _ = send_message(msg);
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();

            // Configure the main window:
            //   1. click-through on by default (the overlay is unusable otherwise).
            //   2. position on primary monitor, bottom-center.
            //   3. install a window event listener that flags geometry changes.
            //   4. start a thin background thread that drains the flag and emits
            //      `position_changed` no more often than every 150ms.
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_ignore_cursor_events(true);
                if let Ok(Some(m)) = w.primary_monitor() {
                    let _ = position_on_monitor_bottom(&w, &m, BOTTOM_MARGIN_PX);
                }

                w.on_window_event(|event| match event {
                    WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                        GEOMETRY_DIRTY.store(true, Ordering::Relaxed);
                    }
                    _ => {}
                });

                // Spawn the position reporter. It owns a clone of the window
                // (cheap — WebviewWindow internally is an Arc-like handle).
                let w_for_reporter = w.clone();
                thread::spawn(move || position_reporter_loop(w_for_reporter));
            }

            // Announce readiness to Chrome extension.
            send(&OutMessage::Ready {
                version: env!("CARGO_PKG_VERSION"),
                platform: std::env::consts::OS,
                capabilities: CAPABILITIES,
            });

            // stdin reader on its own thread (std::io::stdin is blocking).
            thread::spawn(move || stdin_loop(handle));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run failed");
}

fn stdin_loop(app: tauri::AppHandle) {
    loop {
        match read_message() {
            Ok(None) => {
                // stdin closed → Chrome disconnected → exit cleanly.
                // No `goodbye`: the disconnect was Chrome-initiated, the extension
                // already knows the port is gone.
                app.exit(0);
                return;
            }
            Ok(Some(msg)) => handle_message(&app, msg),
            Err(e) => {
                send(&OutMessage::Error {
                    code: "parse",
                    message: e.to_string(),
                });
            }
        }
    }
}

fn handle_message(app: &tauri::AppHandle, msg: InMessage) {
    match msg {
        InMessage::ShowCaption { text, settings } => {
            let _ = app.emit(
                "show-caption",
                serde_json::json!({ "text": text, "settings": settings }),
            );
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
            }
        }
        InMessage::HideCaption => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
        }
        InMessage::UpdateStyle { settings } => {
            let _ = app.emit("update-style", settings);
        }
        InMessage::SetPosition { x, y, width, height } => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_position(PhysicalPosition::new(x, y));
                let _ = w.set_size(PhysicalSize::new(width, height));
            }
        }
        InMessage::SetClickThrough { enabled } => {
            if let Some(w) = app.get_webview_window("main") {
                match w.set_ignore_cursor_events(enabled) {
                    Ok(()) => send(&OutMessage::ClickThrough { enabled }),
                    Err(e) => send(&OutMessage::Error {
                        code: "click_through",
                        message: e.to_string(),
                    }),
                }
            }
        }
        InMessage::SetMonitor { index } => {
            if let Some(w) = app.get_webview_window("main") {
                let monitors = w.available_monitors().unwrap_or_default();
                if let Some(m) = monitors.get(index) {
                    if let Err(e) = position_on_monitor_bottom(&w, m, BOTTOM_MARGIN_PX) {
                        send(&OutMessage::Error {
                            code: "set_monitor",
                            message: e.to_string(),
                        });
                    }
                } else {
                    send(&OutMessage::Error {
                        code: "monitor_out_of_range",
                        message: format!("index {} out of {} monitors", index, monitors.len()),
                    });
                }
            }
        }
        InMessage::ListMonitors => {
            if let Some(w) = app.get_webview_window("main") {
                send(&OutMessage::MonitorList {
                    monitors: collect_monitors(&w),
                });
            }
        }
        InMessage::Ping => {
            send(&OutMessage::Pong);
        }
        InMessage::Exit => {
            // Announce intentional shutdown so the extension's onDisconnect
            // handler can distinguish "we asked for it" from "the host crashed".
            send(&OutMessage::Goodbye {
                reason: "exit_requested",
            });
            app.exit(0);
        }
    }
}

/// Background loop that emits `position_changed` whenever the user has moved
/// or resized the window since the last tick. Runs on its own thread so we
/// never block the Tauri runtime.
fn position_reporter_loop(window: WebviewWindow) {
    let interval = Duration::from_millis(POSITION_REPORT_INTERVAL_MS);
    loop {
        thread::sleep(interval);
        if !GEOMETRY_DIRTY.swap(false, Ordering::Relaxed) {
            continue;
        }
        let pos = match window.outer_position() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let size = match window.outer_size() {
            Ok(s) => s,
            Err(_) => continue,
        };
        send(&OutMessage::PositionChanged {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        });
    }
}

/// Place the window horizontally centered, with a fixed margin from the bottom
/// of the given monitor. Monitors can live at negative coordinates (multi-monitor
/// setups where the primary is not top-left), so we always work in absolute
/// physical coordinates.
fn position_on_monitor_bottom(
    window: &WebviewWindow,
    monitor: &Monitor,
    margin_bottom: i32,
) -> tauri::Result<()> {
    let win_size = window.outer_size()?;
    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let win_w = win_size.width as i32;
    let win_h = win_size.height as i32;
    let mon_w = mon_size.width as i32;
    let mon_h = mon_size.height as i32;
    let x = mon_pos.x + (mon_w - win_w).max(0) / 2;
    let y = mon_pos.y + (mon_h - win_h - margin_bottom).max(0);
    window.set_position(PhysicalPosition::new(x, y))?;
    Ok(())
}

fn collect_monitors(window: &WebviewWindow) -> Vec<MonitorInfo> {
    let primary = window.primary_monitor().ok().flatten();
    let primary_origin = primary.as_ref().map(|m| {
        let p = m.position();
        (p.x, p.y)
    });
    let monitors = window.available_monitors().unwrap_or_default();
    monitors
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let pos = m.position();
            let size = m.size();
            let is_primary = primary_origin
                .map(|(px, py)| px == pos.x && py == pos.y)
                .unwrap_or(false);
            MonitorInfo {
                index: i,
                name: m.name().cloned(),
                x: pos.x,
                y: pos.y,
                width: size.width,
                height: size.height,
                scale_factor: m.scale_factor(),
                is_primary,
            }
        })
        .collect()
}
