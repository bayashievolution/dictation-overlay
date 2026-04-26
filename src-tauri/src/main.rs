#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod native_messaging;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

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
    "transition",
    "writing-mode",
];

/// Bottom margin (px) when auto-positioning on a monitor.
const BOTTOM_MARGIN_PX: i32 = 80;

/// v0.3.9 (起動時暫定) → v0.3.10 で見直し：
/// 暫定サイズが「狭い」と CSS の `max-width: calc(100vw - 32px)` が効いて
/// `.caption` 幅も狭くなり、日本語が文字単位で縦に折り返す → ResizeObserver
/// が「縦長」を観測してさらに細くするフィードバックループに陥った（やっさんの
/// 「縦書きになってる」現象）。
///
/// 対策：起動時暫定はモニタ幅 100%・高さ 25% と広めに取り、`.caption` が
/// 内容幅で自然に inline-block レイアウトされてから ResizeObserver で
/// 縮小するフローに。透明ウィンドウなのでデカく取っても見た目に問題なし、
/// 起動直後はクリックスルー ON なので空き部分も無害。
const WINDOW_WIDTH_RATIO: f64 = 1.0; // モニタ幅 100%（暫定、即追従）
const WINDOW_HEIGHT_RATIO: f64 = 0.25; // モニタ高さ 25%（暫定、即追従）

/// v0.3.9: caption_resized で計算するウィンドウ余白（px）。
/// 字幕の outer サイズ + 余白 がウィンドウサイズ。
/// v0.3.21: 16px だと `.caption` の角丸や outline が WebView 境界に近すぎてクリップされ、
/// ResizeObserver の微小揺れと相まって「上半分が消える」事故が起きた。
/// 32px に倍増して余白を確保、揺れも吸収。
const WINDOW_PADDING_PX: u32 = 32;

/// v0.3.22: caption_resized は set_size 直後この時間内の再 invoke を無視する
/// （クールダウン方式）。これで振動ループを止めつつ、ユーザーがスライダーを
/// 連続的に動かす時もちゃんと追従する（クールダウン後の最終値で set_size）。
const RESIZE_COOLDOWN_MS: u64 = 100;

/// How long to wait between position_changed emits (debounce).
const POSITION_REPORT_INTERVAL_MS: u64 = 150;

/// Serializes concurrent stdout writes.
static STDOUT_LOCK: Mutex<()> = Mutex::new(());

/// Set when the window emits a Moved or Resized event.
/// The reporter thread clears it after sending one `position_changed`.
static GEOMETRY_DIRTY: AtomicBool = AtomicBool::new(false);

/// v0.3.22: 最後に caption_resized で set_size を呼んだ時刻。
/// クールダウン期間中の再 invoke はスキップ → 振動ループ防止。
static LAST_RESIZE_TIME: Mutex<Option<Instant>> = Mutex::new(None);

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

/// v0.3.9: WebView から呼ばれる。`.caption` のサイズが変わるたびに
/// ウィンドウサイズも追従させる。この設計で:
///   ① フォント大やブロック間隔大でもウィンドウ境界でクリップされない
///   ② クリックスルー OFF 時に「字幕より上の透明な空白領域」が消えるので、
///      マウスイベントが下のアプリに自然に届く
/// 位置は「ウィンドウの下端を保つ（縦に伸びる時は上に伸びる）」ため、
/// 既存の outer_position を起点に高さ差分を上に足す。
#[tauri::command]
fn caption_resized(window: WebviewWindow, width: u32, height: u32) {
    // v0.3.22: クールダウン中はスキップ。set_size 直後の WebView リフローで
    // `.caption` のサイズが微変化して再 invoke される振動ループ対策。
    // 「微小しきい値で無視」から「時間ベースで無視」に変えた理由：
    // しきい値方式は「ユーザーがスライダーを連続的に小さく動かす」ケースで
    // 全部 no-op になって追従しない事故が起きた（v0.3.21 検証で発覚）。
    {
        let last = LAST_RESIZE_TIME.lock().unwrap();
        if let Some(t) = *last {
            if t.elapsed() < Duration::from_millis(RESIZE_COOLDOWN_MS) {
                return;
            }
        }
    }

    let win_w = (width + WINDOW_PADDING_PX * 2).max(120);
    let win_h = (height + WINDOW_PADDING_PX * 2).max(60);

    let (old_pos, old_size) = match (window.outer_position(), window.outer_size()) {
        (Ok(p), Ok(s)) => (p, s),
        _ => {
            let _ = window.set_size(PhysicalSize::new(win_w, win_h));
            *LAST_RESIZE_TIME.lock().unwrap() = Some(Instant::now());
            return;
        }
    };

    // 完全同一サイズなら no-op（最低限のループ防止）
    if win_w == old_size.width && win_h == old_size.height {
        return;
    }

    // 下端を保つように y を再計算
    let new_y = old_pos.y + old_size.height as i32 - win_h as i32;
    // x は中央維持（旧 x + (旧幅 - 新幅) / 2）
    let new_x = old_pos.x + (old_size.width as i32 - win_w as i32) / 2;

    let _ = window.set_size(PhysicalSize::new(win_w, win_h));
    let _ = window.set_position(PhysicalPosition::new(new_x, new_y));
    *LAST_RESIZE_TIME.lock().unwrap() = Some(Instant::now());
}

/// v0.3.9: WebView から呼ばれる。fade-out アニメーション完了後に呼ばれて
/// 実際にウィンドウを隠す。`hide_caption` は Rust 側で直接 `window.hide()`
/// せず、まず `fade-out-and-hide` イベントを emit → JS が CSS アニメ後に
/// この command を呼ぶ、という流れ。
#[tauri::command]
fn window_hide(window: WebviewWindow) {
    let _ = window.hide();
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![caption_resized, window_hide])
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
            // v0.3.18: WebView の DevTools を開くメニュー項目（デバッグ用）
            let devtools_item =
                MenuItemBuilder::with_id("open_devtools", "DevTools を開く").build(app)?;
            let exit_item =
                MenuItemBuilder::with_id("exit_overlay", "オーバーレイを終了").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&ct_item)
                .item(&visible_item)
                .separator()
                .item(&report_pos_item)
                .item(&devtools_item)
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
        "open_devtools" => {
            // v0.3.18: WebView の DevTools を開く。Cargo.toml で
            // tauri features に "devtools" 追加済みなのでこの API が使える。
            if let Some(w) = app.get_webview_window("main") {
                w.open_devtools();
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
            // v0.3.9: 直接 hide せず、fade-out イベントを emit。WebView 側で
            // CSS フェード（220ms）を再生してから window_hide コマンドを叩く。
            let _ = app.emit("fade-out-and-hide", ());
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

/// Resize the window to the configured monitor ratio and place it on the
/// bottom-center of the given monitor.
///
/// v0.3.8: 以前は tauri.conf.json で固定サイズ (1200×160) を outer_size として
/// 使ってたが、フォントサイズ大やブロック間隔大で字幕がそこを超えると
/// ウィンドウ境界でクリップされて角丸が切れる問題があった。
/// 今は monitor サイズに対する比率でウィンドウを大きめに取り、字幕は中で
/// flex-end 配置 → 字幕の物理サイズが伸びてもウィンドウ内に収まる。
fn position_on_monitor_bottom(
    window: &WebviewWindow,
    monitor: &Monitor,
    margin_bottom: i32,
) -> tauri::Result<()> {
    let mon_pos = monitor.position();
    let mon_size = monitor.size();
    let mon_w = mon_size.width as i32;
    let mon_h = mon_size.height as i32;

    let win_w = ((mon_w as f64) * WINDOW_WIDTH_RATIO) as i32;
    let win_h = ((mon_h as f64) * WINDOW_HEIGHT_RATIO) as i32;
    window.set_size(PhysicalSize::new(win_w as u32, win_h as u32))?;

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
