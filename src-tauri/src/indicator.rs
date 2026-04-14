use tauri::{AppHandle, Manager};

pub fn show(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        let _ = w.show();
        let _ = w.set_always_on_top(true);
        position_top_right(&w);
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("indicator") {
        let _ = w.hide();
    }
}

fn position_top_right(window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let w = (160.0 * scale) as i32;
        let margin = (20.0 * scale) as i32;
        let x = size.width as i32 - w - margin;
        let y = margin;
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}
