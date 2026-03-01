mod models;
mod modules;
mod commands;
mod utils;
pub mod error;

use tauri::Manager;
use modules::logger;
use tracing::{info, warn, error};

#[derive(Clone, Copy)]
struct AppRuntimeFlags {
    tray_enabled: bool,
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name)
        .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn is_wayland_session() -> bool {
    std::env::var("WAYLAND_DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

fn should_enable_tray() -> bool {
    if env_flag_enabled("HELIX_DISABLE_TRAY") {
        info!("Tray disabled by HELIX_DISABLE_TRAY");
        return false;
    }

    #[cfg(target_os = "linux")]
    {
        if is_wayland_session() && !env_flag_enabled("HELIX_FORCE_TRAY") {
            warn!(
                "Linux Wayland session detected; disabling tray by default to avoid GTK/AppIndicator crashes. Set HELIX_FORCE_TRAY=1 to force-enable."
            );
            return false;
        }
    }

    true
}

#[cfg(target_os = "linux")]
fn configure_linux_gdk_backend() {
    if std::env::var("GDK_BACKEND").is_ok() {
        return;
    }

    let is_wayland = is_wayland_session();
    let has_x11_display = std::env::var("DISPLAY")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false);
    let force_wayland = env_flag_enabled("HELIX_FORCE_WAYLAND");
    let force_x11 = env_flag_enabled("HELIX_FORCE_X11");

    if force_x11 || (is_wayland && has_x11_display && !force_wayland) {
        std::env::set_var("GDK_BACKEND", "x11");
        warn!(
            "Forcing GDK_BACKEND=x11 for stability on Wayland. Set HELIX_FORCE_WAYLAND=1 to keep Wayland backend."
        );
    }
}

/// Increase file descriptor limit for macOS to prevent "Too many open files" errors
#[cfg(target_os = "macos")]
fn increase_nofile_limit() {
    unsafe {
        let mut rl = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) == 0 {
            info!("Current open file limit: soft={}, hard={}", rl.rlim_cur, rl.rlim_max);

            let target = 4096.min(rl.rlim_max);
            if rl.rlim_cur < target {
                rl.rlim_cur = target;
                if libc::setrlimit(libc::RLIMIT_NOFILE, &rl) == 0 {
                    info!("Successfully increased hard file limit to {}", target);
                } else {
                    warn!("Failed to increase file descriptor limit");
                }
            }
        }
    }
}

// Test command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Increase file descriptor limit (macOS only)
    #[cfg(target_os = "macos")]
    increase_nofile_limit();

    // Initialize logger
    logger::init_logger();

    #[cfg(target_os = "linux")]
    configure_linux_gdk_backend();

    let tray_enabled = should_enable_tray();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::VISIBLE
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED
                        | tauri_plugin_window_state::StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.get_webview_window("main")
                .map(|window| {
                    let _ = window.show();
                    let _ = window.set_focus();
                    #[cfg(target_os = "macos")]
                    app.set_activation_policy(tauri::ActivationPolicy::Regular).unwrap_or(());
                });
        }))
        .manage(commands::cloudflared::CloudflaredState::new())
        .manage(AppRuntimeFlags { tray_enabled })
        .setup(|app| {
            info!("Setup starting...");

            // Initialize database
            if let Err(e) = modules::database::init_db() {
                error!("Failed to initialize database: {}", e);
            }

            // Initialize cron tables
            if let Err(e) = modules::cron::init_cron_tables() {
                error!("Failed to initialize cron tables: {}", e);
            }

            // Start skills hot-reload watcher (scans ~/.helix/skills/ every 5s)
            modules::skills::start_skills_watcher();

            // Load user-defined environment variables from ~/.helix/envs.json
            modules::environments::apply_envs_to_process();

            // Initialize hooks tables
            if let Err(e) = modules::hooks::init_hooks_tables() {
                error!("Failed to initialize hooks tables: {}", e);
            }

            // Initialize advanced memory tables
            if let Err(e) = modules::memory::init_memory_tables() {
                error!("Failed to initialize memory tables: {}", e);
            }

            // Initialize session tables
            if let Err(e) = modules::sessions::init_session_tables() {
                error!("Failed to initialize session tables: {}", e);
            }

            // Initialize usage tables
            if let Err(e) = modules::usage::init_usage_tables() {
                error!("Failed to initialize usage tables: {}", e);
            }

            // Initialize log bridge with app handle for debug console
            modules::log_bridge::init_log_bridge(app.handle().clone());

            // Linux: Workaround for transparent window crash/freeze
            #[cfg(target_os = "linux")]
            {
                use tauri::Manager;
                if is_wayland_session() {
                    info!("Linux Wayland session detected; skipping transparent window workaround");
                } else if let Some(window) = app.get_webview_window("main") {
                    if let Ok(gtk_window) = window.gtk_window() {
                        use gtk::prelude::WidgetExt;
                        if let Some(screen) = gtk_window.screen() {
                            if let Some(visual) = screen.system_visual() {
                                gtk_window.set_visual(Some(&visual));
                            }
                            info!("Linux: Applied transparent window workaround");
                        }
                    }
                }
            }

            let runtime_flags = app.state::<AppRuntimeFlags>();
            if runtime_flags.tray_enabled {
                modules::tray::create_tray(app.handle())?;
                info!("Tray created");
            } else {
                info!("Tray disabled for this session");
            }

            // Start smart scheduler
            modules::scheduler::start_scheduler(Some(app.handle().clone()));

            // Start cron job scheduler
            modules::cron::start_cron_scheduler();

            // Start heartbeat system (reads ~/.helix/HEARTBEAT.md periodically)
            modules::cron::start_heartbeat();

            // Start embedded HTTP API server with Swagger UI
            modules::api_server::start_api_server(9520);

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let tray_enabled = window
                    .app_handle()
                    .try_state::<AppRuntimeFlags>()
                    .map(|flags| flags.tray_enabled)
                    .unwrap_or(true);

                if tray_enabled {
                    let _ = window.hide();
                    #[cfg(target_os = "macos")]
                    {
                        use tauri::Manager;
                        window
                            .app_handle()
                            .set_activation_policy(tauri::ActivationPolicy::Accessory)
                            .unwrap_or(());
                    }
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // Config commands
            commands::load_config,
            commands::save_config,
            // Utility commands
            commands::save_text_file,
            commands::read_text_file,
            commands::clear_log_cache,
            commands::show_main_window,
            commands::set_window_theme,
            // Update commands
            commands::check_for_updates,
            commands::check_homebrew_installation,
            commands::brew_upgrade_cask,
            commands::get_update_settings,
            commands::save_update_settings,
            commands::should_check_updates,
            commands::update_last_check_time,
            // Autostart commands
            commands::autostart::toggle_auto_launch,
            commands::autostart::is_auto_launch_enabled,
            // Cloudflared commands
            commands::cloudflared::cloudflared_check,
            commands::cloudflared::cloudflared_install,
            commands::cloudflared::cloudflared_start,
            commands::cloudflared::cloudflared_stop,
            commands::cloudflared::cloudflared_get_status,
            // Debug console commands
            modules::log_bridge::enable_debug_console,
            modules::log_bridge::disable_debug_console,
            modules::log_bridge::is_debug_console_enabled,
            modules::log_bridge::get_debug_console_logs,
            modules::log_bridge::clear_debug_console_logs,
            // K8s / Aliyun config commands
            commands::get_kube_info,
            commands::get_aliyun_info,

            // AI Chat commands
            modules::ai_chat::ai_chat_send,
            modules::ai_chat::ai_get_config,
            modules::ai_chat::ai_set_config,
            modules::ai_chat::ai_test_connection,
            modules::ai_chat::ai_list_models,
            // Database commands
            modules::database::db_list_accounts,
            modules::database::db_get_messages,
            modules::database::db_set_account_remark,
            modules::database::db_set_auto_reply,
            // Agent commands
            modules::agent::agent_chat,
            modules::agent::agent_cancel,
            modules::agent::save_file_to,
            modules::agent::agent_get_history,
            modules::agent::agent_clear_history,
            // Cron commands
            modules::cron::cron_list_tasks,
            modules::cron::cron_create_task,
            modules::cron::cron_update_task,
            modules::cron::cron_delete_task,
            modules::cron::cron_run_task,
            modules::cron::cron_get_runs,
            modules::cron::cron_validate_expr,
            // Notification commands
            modules::notifications::notification_test_send,
            // Skills commands
            modules::skills::skills_list,
            modules::skills::skills_toggle,
            modules::skills::skills_reload,
            modules::skills::skills_get_body,
            modules::skills::skills_create,
            modules::skills::skills_uninstall,
            modules::skills::skills_install_git,
            modules::skills::skills_hub_install,
            modules::skills::skills_open_dir,
            modules::skills::skills_get_dir,
            // Hooks commands
            modules::hooks::hooks_list,
            modules::hooks::hooks_create,
            modules::hooks::hooks_toggle,
            modules::hooks::hooks_delete,
            // Commands
            modules::commands::commands_list,
            modules::commands::commands_execute,
            // Advanced Memory
            modules::memory::memory_search,
            modules::memory::memory_store_entry,
            modules::memory::memory_delete,
            modules::memory::memory_list,
            modules::memory::memory_stats,
            modules::memory::memory_embed,
            modules::memory::memory_save_conversation,
            modules::memory::memory_flush,
            modules::memory::memory_list_files,
            // Security
            modules::security::security_audit,
            // Link Understanding
            modules::link_understanding::link_fetch,
            modules::link_understanding::link_detect,
            modules::link_understanding::link_process,
            // Channels
            modules::channels::channels_list,
            modules::channels::channels_send,
            modules::channels::channels_resolve,
            // Sessions
            modules::sessions::sessions_list,
            modules::sessions::sessions_get,
            modules::sessions::sessions_set_model,
            modules::sessions::sessions_set_policy,
            modules::sessions::sessions_set_label,
            modules::sessions::sessions_delete,
            modules::sessions::sessions_compact,
            // Messaging
            modules::messaging::messaging_chunk,
            modules::messaging::messaging_template,
            // Media Understanding
            modules::media_understanding::media_detect_mime,
            modules::media_understanding::media_extract_file,
            modules::media_understanding::media_describe_image,
            modules::media_understanding::media_transcribe_audio,
            // Providers
            modules::providers::providers_detect,
            modules::providers::providers_resolve,
            // Streaming
            modules::streaming::streaming_test,
            // Usage
            modules::usage::usage_dashboard,
            modules::usage::usage_totals,
            modules::usage::usage_today,
            modules::usage::usage_session,
            modules::usage::usage_by_model,
            modules::usage::usage_daily,
            modules::usage::usage_log,
            modules::usage::usage_estimate_cost,
            // Model Selection
            modules::model_selection::model_resolve,
            modules::model_selection::model_list_aliases,
            modules::model_selection::model_default,
            // Stream Events
            modules::stream_events::stream_clean_text,
            modules::stream_events::stream_strip_thinking,
            // EvoMap
            modules::evomap::evomap_hello,
            modules::evomap::evomap_fetch,
            modules::evomap::evomap_publish,
            modules::evomap::evomap_list_assets,
            modules::evomap::evomap_status,
            modules::evomap::evomap_toggle,
            // Agent Tools
            modules::agent_tools::tool_image_describe,
            // Subagents
            modules::subagents::spawn_subagent,
            modules::subagents::spawn_subagents_batch,
            // Workspace
            modules::workspace::workspace_list_files,
            modules::workspace::workspace_read_file,
            modules::workspace::workspace_write_file,
            modules::workspace::workspace_delete_file,
            modules::workspace::workspace_get_dir,
            // Environments
            modules::environments::envs_list,
            modules::environments::envs_set,
            modules::environments::envs_delete,
            // MCP
            modules::mcp::mcp_list,
            modules::mcp::mcp_create,
            modules::mcp::mcp_toggle,
            modules::mcp::mcp_delete,
            modules::mcp::mcp_update,

        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            match event {
                tauri::RunEvent::Exit => {
                    tracing::info!("Application exiting, cleaning up background tasks...");
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                        app_handle.set_activation_policy(tauri::ActivationPolicy::Regular).unwrap_or(());
                    }
                }
                _ => {}
            }
        });
}
