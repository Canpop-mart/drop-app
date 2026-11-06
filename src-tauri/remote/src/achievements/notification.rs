//! Opt-in windowed/borderless Windows notification prototype. No graphics hooks or focus capture.
use super::journal::Record;

pub fn enqueue(record: &Record) {
    if record.event.source == "steam-native"
        || !database::borrow_db_checked()
            .settings
            .achievement_game_popups
    {
        return;
    }
    let record = record.clone();
    tauri::async_runtime::spawn(async move {
        if !record.scope.is_current() {
            return;
        }
        if let Err(e) = show(&record.title, &record.description, false).await {
            super::tracking::notification_error(&record.scope, &record.game_id, e);
        }
    });
}

pub async fn preview() -> Result<(), String> {
    show(
        "Achievement popup preview",
        "This popup should appear without taking focus.",
        true,
    )
    .await
}

#[cfg(not(windows))]
async fn show(_title: &str, _description: &str, _preview: bool) -> Result<(), String> {
    Err("Game popups are currently available on Windows. Use the game's or emulator's native notifications on this device.".into())
}

#[cfg(windows)]
async fn show(title: &str, description: &str, preview: bool) -> Result<(), String> {
    use std::{sync::Mutex, time::Duration};
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
    static POPUP: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = POPUP.lock().await;
    let handle = crate::utils::DROP_APP_HANDLE
        .lock()
        .await
        .clone()
        .ok_or("Drop's window is unavailable.")?;
    let main = handle
        .get_window("main")
        .ok_or("Drop's window is unavailable.")?;
    if !preview && main.is_focused().map_err(|e| e.to_string())? {
        return Ok(());
    }
    let monitor = main
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("The display is unavailable.")?;
    let scale = monitor.scale_factor();
    let x = (monitor.position().x as f64 + monitor.size().width as f64) / scale - 420.0;
    let y = (monitor.position().y as f64 + monitor.size().height as f64) / scale - 180.0;
    let data = serde_json::json!({"title":title,"description":description}).to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tx = Mutex::new(Some(tx));
    let popup = WebviewWindowBuilder::new(
        &handle,
        "achievement-popup",
        WebviewUrl::App("achievement-popup.html".into()),
    )
    .title("Drop achievement")
    .inner_size(396.0, 132.0)
    .position(x, y)
    .decorations(false)
    .shadow(false)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .focusable(false)
    .visible(false)
    .transparent(true)
    .on_page_load(move |window, payload| {
        if payload.event() != tauri::webview::PageLoadEvent::Finished {
            return;
        }
        let result = window
            .eval(format!("window.showAchievement({data});"))
            .and_then(|_| window.show())
            .map_err(|e| e.to_string());
        if let Ok(mut sender) = tx.lock() {
            if let Some(sender) = sender.take() {
                let _ = sender.send(result);
            }
        }
    })
    .build()
    .map_err(|e| e.to_string())?;
    if let Err(e) = popup.set_ignore_cursor_events(true) {
        let _ = popup.destroy();
        return Err(e.to_string());
    }
    let loaded = tokio::time::timeout(Duration::from_secs(5), rx).await;
    let result = match loaded {
        Ok(Ok(result)) => result,
        _ => Err("The achievement popup did not load.".into()),
    };
    if result.is_ok() {
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    let _ = popup.destroy();
    result
}
