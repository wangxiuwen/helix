use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Manager, Listener,
};
use crate::modules;

pub fn create_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    // 1. Load config to get language settings
    let config = modules::load_app_config().unwrap_or_default();
    let texts = modules::i18n::get_tray_texts(&config.language);
    
    // 2. Load icon
    let icon_bytes = include_bytes!("../../icons/tray-icon.png");
    let img = image::load_from_memory(icon_bytes)
        .map_err(|e| tauri::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?
        .to_rgba8();
    let (width, height) = img.dimensions();
    let icon = Image::new_owned(img.into_raw(), width, height);

    // 3. Define menu items
    let show_i = MenuItem::with_id(app, "show", &texts.show_window, true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", &texts.quit, true, None::<&str>)?;
    
    let sep = PredefinedMenuItem::separator(app)?;

    // 4. Build menu
    let menu = Menu::with_items(app, &[
        &show_i,
        &sep,
        &quit_i,
    ])?;

    // 5. Build tray icon
    let _ = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(icon)
        .on_menu_event(move |app, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                        #[cfg(target_os = "macos")]
                        app.set_activation_policy(tauri::ActivationPolicy::Regular).unwrap_or(());
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
               let app = tray.app_handle();
               if let Some(window) = app.get_webview_window("main") {
                   let _ = window.show();
                   let _ = window.set_focus();
                   #[cfg(target_os = "macos")]
                   app.set_activation_policy(tauri::ActivationPolicy::Regular).unwrap_or(());
               }
            }
        })
        .build(app)?;

    // Listen for config update events
    let handle = app.clone();
    app.listen("config://updated", move |_event| {
        modules::logger::log_info("Configuration updated, refreshing tray menu");
        update_tray_menus(&handle);
    });

    Ok(())
}

/// Helper function to update tray menu
pub fn update_tray_menus(app: &tauri::AppHandle) {
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
         let config = modules::load_app_config().unwrap_or_default();
         let texts = modules::i18n::get_tray_texts(&config.language);
         
         let show_i = MenuItem::with_id(&app_clone, "show", &texts.show_window, true, None::<&str>);
         let quit_i = MenuItem::with_id(&app_clone, "quit", &texts.quit, true, None::<&str>);
         
         if let (Ok(s), Ok(q)) = (show_i, quit_i) {
             let sep = PredefinedMenuItem::separator(&app_clone).ok();
             
             let mut items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = vec![&s];
             if let Some(ref sep) = sep { items.push(sep); }
             items.push(&q);
             
             if let Ok(menu) = Menu::with_items(&app_clone, &items) {
                 if let Some(tray) = app_clone.tray_by_id("main") {
                     let _ = tray.set_menu(Some(menu));
                 }
             }
         }
    });
}
