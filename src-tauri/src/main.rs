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
    "transition",
    "writing-mode",
    "stream-mode",
    // v0.4.0: 自動クリックスルー（マウスポーリングで .caption 領域だけ反応）
    "click-through-auto",
];

/// Bottom margin (px) when auto-positioning on a monitor.
const BOTTOM_MARGIN_PX: i32 = 80;

/// v0.3.23: ウィンドウサイズを「動的追従」から「固定」に戻した。
const WINDOW_WIDTH_RATIO: f64 = 1.0;
const WINDOW_HEIGHT_RATIO: f64 = 0.5;

/// How long to wait between position_changed emits (debounce).
const POSITION_REPORT_INTERVAL_MS: u64 = 150;

/// v0.4.0: 自動クリックスルーのマウスポーリング間隔。
const MOUSE_POLL_INTERVAL_MS: u64 = 50;

/// Serializes concurrent stdout writes.
static STDOUT_LOCK: Mutex<()> = Mutex::new(());

/// Set when the window emits a Moved or Resized event.
static GEOMETRY_DIRTY: AtomicBool = AtomicBool::new(false);

/// v0.4.0: 自動クリックスルーモードでの「最後に適用した ignore_cursor_events 値」キャッシュ。
/// 50ms ごとに同じ値を再 set すると無駄なので差分があるときだけ呼ぶ。
static LAST_AUTO_IGNORE: AtomicBool = AtomicBool::new(true);

/// v0.4.0: クリックスルーモード。
/// - `Auto`: マウスポーリングで `.caption` 領域内なら ignore=false, 外なら ignore=true
/// - `ForceOn`: 強制 ON（窓全体がクリックを下のアプリに流す。従来 `set_click_through{enabled:true}`）
/// - `ForceOff`: 強制 OFF（窓全体がクリックを捕捉。位置調整等の特殊用途）
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClickThroughMode {
    Auto,
    ForceOn,
    ForceOff,
}

impl ClickThroughMode {
    fn as_str(&self) -> &'static str {
        match self {
            ClickThroughMode::Auto => "auto",
            ClickThroughMode::ForceOn => "force_on",
            ClickThroughMode::ForceOff => "force_off",
        }
    }

    /// 「外から見えるクリックスルー有効/無効」の Boolean 表現。
    /// `Auto` は実質「ほぼ ON、字幕の上でだけ OFF」なので true 扱いで `ClickThrough` イベントを返す。
    /// 古い拡張への後方互換用。
    fn legacy_enabled(&self) -> bool {
        !matches!(self, ClickThroughMode::ForceOff)
    }
}

/// グローバルモード状態。デフォルトは v0.4.0 から自動。
static CLICK_THROUGH_MODE: Mutex<ClickThroughMode> = Mutex::new(ClickThroughMode::Auto);

/// v0.4.0: WebView から push された `.caption` の bounding rect（CSS px、viewport 相対）。
/// 50ms ポーリング側でこれを `inner_position + scale_factor` を使って物理スクリーン座標へ変換し、
/// マウス位置と照合する。`None` のときは「無効領域」扱い → ignore=true 固定。
#[derive(Clone, Copy)]
struct RectCss {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}
static CAPTION_RECT_CSS: Mutex<Option<RectCss>> = Mutex::new(None);

/// Holds clones of the tray menu's check items so we can sync their visual
/// state when the underlying state changes via Native Messaging.
struct MenuHandles {
    ct_auto: CheckMenuItem<Wry>,
    ct_force_on: CheckMenuItem<Wry>,
    ct_force_off: CheckMenuItem<Wry>,
    visible: CheckMenuItem<Wry>,
}

fn send(msg: &OutMessage) {
    let _g = STDOUT_LOCK.lock().ok();
    let _ = send_message(msg);
}

/// v0.4.0: WebView から `.caption.getBoundingClientRect()` を push する。
/// `w` または `h` が 0 以下のとき（隠れている／消えた等）は `None` 扱い。
#[tauri::command]
fn caption_rect(x: f64, y: f64, w: f64, h: f64) {
    let mut g = CAPTION_RECT_CSS.lock().expect("rect mutex poisoned");
    *g = if w > 0.0 && h > 0.0 {
        Some(RectCss { x, y, w, h })
    } else {
        None
    };
}

/// v0.3.9: WebView から呼ばれる。fade-out アニメーション完了後に呼ばれて
/// 実際にウィンドウを隠す。`hide_caption` は Rust 側で直接 `window.hide()`
/// せず、まず `fade-out-and-hide` イベントを emit → JS が CSS アニメ後に
/// この command を呼ぶ、という流れ。
#[tauri::command]
fn window_hide(window: WebviewWindow) {
    let _ = window.hide();
    // v0.4.0: 隠した直後は rect も無効化（ポーリングが古い rect で誤判定するのを防ぐ）
    *CAPTION_RECT_CSS.lock().expect("rect mutex poisoned") = None;
}

/// v0.4.0: 左マウスボタンが現在押されているか。Windows のみ。
/// ドラッグ中（`data-tauri-drag-region` で字幕窓を引きずっている最中）に
/// `set_ignore_cursor_events` をトグルすると OS のドラッグキャプチャが切れる
/// 可能性があるので、ボタン押下中はモード遷移を凍結する。
#[cfg(windows)]
fn left_mouse_button_pressed() -> bool {
    extern "system" {
        fn GetAsyncKeyState(v_key: i32) -> i16;
    }
    const VK_LBUTTON: i32 = 0x01;
    // 最上位ビット = 現在押されている
    unsafe { (GetAsyncKeyState(VK_LBUTTON) as u16) & 0x8000 != 0 }
}

#[cfg(not(windows))]
fn left_mouse_button_pressed() -> bool {
    false
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![window_hide, caption_rect])
        .setup(|app| {
            // ---- Window setup --------------------------------------------
            if let Some(w) = app.get_webview_window("main") {
                // v0.4.0: 起動直後は ignore=true（カーソルが下のアプリに通る）。
                // .caption の rect が来てから自動モードのポーリングが ignore=false に切り替える。
                let _ = w.set_ignore_cursor_events(true);
                LAST_AUTO_IGNORE.store(true, Ordering::Relaxed);
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

            // v0.4.0: マウスポーリングスレッド
            let app_for_poll = app.handle().clone();
            thread::spawn(move || mouse_polling_loop(app_for_poll));

            // ---- Tray icon + menu ---------------------------------------
            // v0.4.0: クリックスルーは 3 モードラジオに変更。
            //   - 自動 (字幕の上だけ反応)  ← デフォルト
            //   - 強制 ON (全領域がクリックを通す)
            //   - 強制 OFF (全領域でクリックを捕捉)
            let ct_auto = CheckMenuItemBuilder::with_id(
                "ct_auto",
                "クリックスルー: 自動 (字幕の上だけ反応)",
            )
            .checked(true)
            .build(app)?;
            let ct_force_on =
                CheckMenuItemBuilder::with_id("ct_force_on", "クリックスルー: 強制 ON")
                    .checked(false)
                    .build(app)?;
            let ct_force_off =
                CheckMenuItemBuilder::with_id("ct_force_off", "クリックスルー: 強制 OFF")
                    .checked(false)
                    .build(app)?;

            let visible_item = CheckMenuItemBuilder::with_id("visible", "字幕を表示中")
                .checked(false)
                .build(app)?;
            let report_pos_item =
                MenuItemBuilder::with_id("report_pos", "現在位置を通知").build(app)?;
            let devtools_item =
                MenuItemBuilder::with_id("open_devtools", "DevTools を開く").build(app)?;
            let exit_item =
                MenuItemBuilder::with_id("exit_overlay", "オーバーレイを終了").build(app)?;

            let menu = MenuBuilder::new(app)
                .item(&ct_auto)
                .item(&ct_force_on)
                .item(&ct_force_off)
                .separator()
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

            let _tray = TrayIconBuilder::with_id("main")
                .tooltip("dictation-overlay")
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    handle_tray_event(app, event.id().as_ref());
                })
                .build(app)?;

            app.manage(MenuHandles {
                ct_auto,
                ct_force_on,
                ct_force_off,
                visible: visible_item,
            });

            // ---- Announce + start stdin loop -----------------------------
            send(&OutMessage::Ready {
                version: env!("CARGO_PKG_VERSION"),
                platform: std::env::consts::OS,
                capabilities: CAPABILITIES,
            });
            // v0.4.0: 起動直後の現在モード（既定 = auto）も通知
            send(&OutMessage::ClickThroughMode { mode: "auto" });

            let handle = app.handle().clone();
            thread::spawn(move || stdin_loop(handle));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run failed");
}

fn handle_tray_event(app: &tauri::AppHandle, id: &str) {
    match id {
        "ct_auto" => apply_mode(app, ClickThroughMode::Auto),
        "ct_force_on" => apply_mode(app, ClickThroughMode::ForceOn),
        "ct_force_off" => apply_mode(app, ClickThroughMode::ForceOff),
        "visible" => {
            if let Some(handles) = app.try_state::<MenuHandles>() {
                let new_state = handles.visible.is_checked().unwrap_or(false);
                if let Some(w) = app.get_webview_window("main") {
                    if new_state {
                        let _ = w.show();
                    } else {
                        let _ = w.hide();
                    }
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
            if let Some(w) = app.get_webview_window("main") {
                w.open_devtools();
            }
        }
        "exit_overlay" => {
            send(&OutMessage::Goodbye {
                reason: "user_close",
            });
            app.exit(0);
        }
        _ => {}
    }
}

/// v0.4.0: クリックスルーモードを切り替える。
/// - グローバル状態を更新
/// - トレイメニューのチェック状態を 3 つラジオ的に同期
/// - ForceOn / ForceOff は即座に `set_ignore_cursor_events` を確定
/// - Auto はポーリング側に任せるが、現在のキャッシュをリセットして次の tick で必ず差分発火させる
/// - 拡張に `ClickThroughMode` と（後方互換のため）`ClickThrough` を送る
fn apply_mode(app: &tauri::AppHandle, mode: ClickThroughMode) {
    {
        let mut g = CLICK_THROUGH_MODE.lock().expect("mode mutex poisoned");
        *g = mode;
    }
    if let Some(handles) = app.try_state::<MenuHandles>() {
        let _ = handles.ct_auto.set_checked(mode == ClickThroughMode::Auto);
        let _ = handles
            .ct_force_on
            .set_checked(mode == ClickThroughMode::ForceOn);
        let _ = handles
            .ct_force_off
            .set_checked(mode == ClickThroughMode::ForceOff);
    }
    if let Some(w) = app.get_webview_window("main") {
        match mode {
            ClickThroughMode::Auto => {
                // 次の tick で必ず再評価されるよう「無効値」をキャッシュに入れる。
                // 起動直後の状態は ignore=true なので、わざと反対の false を入れて
                // 「次のループで何があっても 1 回 set する」状態にする。
                LAST_AUTO_IGNORE.store(false, Ordering::Relaxed);
            }
            ClickThroughMode::ForceOn => {
                let _ = w.set_ignore_cursor_events(true);
                LAST_AUTO_IGNORE.store(true, Ordering::Relaxed);
            }
            ClickThroughMode::ForceOff => {
                let _ = w.set_ignore_cursor_events(false);
                LAST_AUTO_IGNORE.store(false, Ordering::Relaxed);
            }
        }
    }
    send(&OutMessage::ClickThroughMode {
        mode: mode.as_str(),
    });
    // 古い拡張向け後方互換：「外から見たクリックスルー有効/無効」を boolean で。
    send(&OutMessage::ClickThrough {
        enabled: mode.legacy_enabled(),
    });
}

fn stdin_loop(app: tauri::AppHandle) {
    loop {
        match read_message() {
            Ok(None) => {
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
            let _ = app.emit("fade-out-and-hide", ());
            sync_visible(app, false);
        }
        InMessage::UpdateStyle { settings } => {
            let _ = app.emit("update-style", settings);
        }
        InMessage::SetPosition {
            x,
            y,
            width,
            height,
        } => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_position(PhysicalPosition::new(x, y));
                let _ = w.set_size(PhysicalSize::new(width, height));
            }
        }
        InMessage::SetClickThrough { enabled, auto } => {
            // v0.4.0: 3 モード対応。
            //   - auto=Some(true)             → Auto
            //   - auto=Some(false) or None で enabled=true   → ForceOn
            //   - auto=Some(false) or None で enabled=false  → ForceOff
            let mode = match auto {
                Some(true) => ClickThroughMode::Auto,
                _ => {
                    if enabled {
                        ClickThroughMode::ForceOn
                    } else {
                        ClickThroughMode::ForceOff
                    }
                }
            };
            apply_mode(app, mode);
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

fn sync_visible(app: &tauri::AppHandle, visible: bool) {
    if let Some(handles) = app.try_state::<MenuHandles>() {
        let _ = handles.visible.set_checked(visible);
    }
}

/// Background loop that emits `position_changed` whenever the user has moved
/// or resized the window since the last tick.
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

/// v0.4.0: 50ms 周期でカーソル位置を取り、自動モードなら `.caption` 領域内/外で
/// `set_ignore_cursor_events` を切り替える。
///
/// **設計のキモ**：
/// - JS 側は `.caption` の `getBoundingClientRect()` を CSS px で push してくる
/// - Rust 側のループで毎回 `window.inner_position()` と `scale_factor()` を見て
///   物理スクリーン座標に変換する → ウィンドウ移動時に JS push 不要
/// - 左ボタン押下中はモード遷移を凍結（ドラッグキャプチャを切らない保険）
/// - 差分があるときだけ `set_ignore_cursor_events` を呼ぶ（無駄 API 抑制）
fn mouse_polling_loop(app: tauri::AppHandle) {
    let interval = Duration::from_millis(MOUSE_POLL_INTERVAL_MS);
    loop {
        thread::sleep(interval);

        // 自動モード以外はスキップ
        let mode = *CLICK_THROUGH_MODE.lock().expect("mode mutex poisoned");
        if mode != ClickThroughMode::Auto {
            continue;
        }
        // ドラッグ中は凍結
        if left_mouse_button_pressed() {
            continue;
        }
        let window = match app.get_webview_window("main") {
            Some(w) => w,
            None => continue,
        };
        let cursor = match app.cursor_position() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let inner_pos = match window.inner_position() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let scale = window.scale_factor().unwrap_or(1.0);
        let rect_css = *CAPTION_RECT_CSS.lock().expect("rect mutex poisoned");

        let want_ignore = match rect_css {
            None => true, // 字幕領域不明 → 全部素通し（auto=true）
            Some(r) => {
                let abs_x = inner_pos.x as f64 + r.x * scale;
                let abs_y = inner_pos.y as f64 + r.y * scale;
                let abs_w = r.w * scale;
                let abs_h = r.h * scale;
                let inside = cursor.x >= abs_x
                    && cursor.x < abs_x + abs_w
                    && cursor.y >= abs_y
                    && cursor.y < abs_y + abs_h;
                !inside
            }
        };

        let prev = LAST_AUTO_IGNORE.swap(want_ignore, Ordering::Relaxed);
        if prev != want_ignore {
            let _ = window.set_ignore_cursor_events(want_ignore);
        }
    }
}

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
