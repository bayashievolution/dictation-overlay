#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod native_messaging;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::{
    Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent, Wry,
};

use native_messaging::{read_message, send_message, InMessage, MonitorInfo, OutMessage};

const CAPABILITIES: &[&str] = &[
    "transparency",
    "always-on-top",
    "click-through",
    "multi-monitor",
    "position-report",
    "tray-menu",
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

/// Holds clones of the tray menu's check items so we can sync their visual
/// state when the underlying state changes via Native Messaging.
struct MenuHandles {
    click_through: CheckMenuItem<Wry>,
    visible: CheckMenuItem<Wry>,
}

fn send(msg: &OutMessage) {
    let _g = STDOUT_LOCK.lock().ok();
    let _ = send_message(msg);
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // ---- Window setup --------------------------------------------
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

                let w_for_reporter = w.clone();
                thread::spawn(move || position_reporter_loop(w_for_reporter));
            }

            // ---- Tray icon + menu ---------------------------------------
            let ct_item = CheckMenuItemBuilder::with_id("ct", "クリックスルー有効")
                .checked(true)
                .build(app)?;
            let visible_item = CheckMenuItemBuilder::with_id("visible", "字幕を表示中")
                .checked(false)
                .build(app)?;
            let report_pos_item =
                MenuItemBuilder::with_id("report_pos", "現在位置を通知").build(app)?;
            let exit_item =
                MenuItemBuilder::with_id("exit_overlay", "オーバーレイを終了").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&ct_item)
                .item(&visible_item)
                .separator()
                .item(&report_pos_item)
                .separator()
                .item(&exit_item)
                .build()?;

            let icon = app
                .default_window_icon()
                .cloned()
                .expect("default window icon must be set in tauri.conf.json");

            let ct_for_handler = ct_item.clone();
            let visible_for_handler = visible_item.clone();
            let _tray = TrayIconBuilder::with_id("main")
                .tooltip("dictation-overlay")
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    handle_tray_event(app, event.id().as_ref(), &ct_for_handler, &visible_for_handler);
                })
                .build(app)?;

            // Stash the check-item handles so handle_message can keep the
            // tray menu's visual state in sync with Native-Messaging-driven changes.
            app.manage(MenuHandles {
                click_through: ct_item,
                visible: visible_item,
            });

            // ---- Announce + start stdin loop -----------------------------
            send(&OutMessage::Ready {
                version: env!("CARGO_PKG_VERSION"),
                platform: std::env::consts::OS,
                capabilities: CAPABILITIES,
            });

            let handle = app.handle().clone();
            thread::spawn(move || stdin_loop(handle));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run failed");
}

fn handle_tray_event(
    app: &tauri::AppHandle,
    id: &str,
    ct: &CheckMenuItem<Wry>,
    visible: &CheckMenuItem<Wry>,
) {
    match id {
        "ct" => {
            // Tauri auto-toggles the check state on click; read it and apply.
            let new_state = ct.is_checked().unwrap_or(true);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_ignore_cursor_events(new_state);
            }
            send(&OutMessage::ClickThrough { enabled: new_state });
        }
        "visible" => {
            let new_state = visible.is_checked().unwrap_or(false);
            if let Some(w) = app.get_webview_window("main") {
                if new_state {
                    let _ = w.show();
                } else {
                    let _ = w.hide();
                }
            }
        }
        "report_pos" => {
            if let Some(w) = app.get_webview_window("main") {
                if let (Ok(pos), Ok(size)) = (w.outer_position(), w.outer_size()) {
                    send(&OutMessage::PositionChanged {
                        x: pos.x,
                        y: pos.y,
                        width: size.width,
                        height: size.height,
                    });
                }
            }
        }
        "exit_overlay" => {
            send(&OutMessage::Goodbye { reason: "user_close" });
            app.exit(0);
        }
        _ => {}
    }
}

fn stdin_loop(app: tauri::AppHandle) {
    loop {
        match read_message() {
            Ok(None) => {
                // stdin closed → Chrome disconnected → exit cleanly.
                // No `goodbye`: the disconnect was Chrome-initiated.
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
            sync_visible(app, true);
        }
        InMessage::HideCaption => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.hide();
            }
            sync_visible(app, false);
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
                    Ok(()) => {
                        send(&OutMessage::ClickThrough { enabled });
                        sync_click_through(app, enabled);
                    }
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
            send(&OutMessage::Goodbye {
                reason: "exit_requested",
            });
            app.exit(0);
        }
    }
}

fn sync_click_through(app: &tauri::AppHandle, enabled: bool) {
    if let Some(handles) = app.try_state::<MenuHandles>() {
        let _ = handles.click_through.set_checked(enabled);
    }
}

fn sync_visible(app: &tauri::AppHandle, visible: bool) {
    if let Some(handles) = app.try_state::<MenuHandles>() {
        let _ = handles.visible.set_checked(visible);
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
/// of the given monitor.
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
