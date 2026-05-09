// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    {
        // WebKit2GTK's DMA-BUF renderer causes blurry text/UI under fractional
        // scaling (125 %, 150 %, …) on Linux/Wayland and with certain GPU drivers.
        // Disabling it forces the legacy renderer which respects sub-pixel layout.
        // https://github.com/tauri-apps/tauri/issues/14590
        if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }
    left_hand_control_lib::run()
}
