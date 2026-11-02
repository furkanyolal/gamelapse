pub mod commands;
pub mod core;
pub mod error;
pub mod runtime;
pub mod state;

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, RwLock};

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::state::{AppState, LogGuard};

const MAIN_WINDOW: &str = "main";

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
        commands::get_app_info,
        commands::ping,
        commands::tracker::get_tracker_status,
        commands::tracker::set_tracking_enabled,
        commands::tracker::start_manual_session,
        commands::tracker::stop_session,
        commands::games::list_games,
        commands::games::get_game,
        commands::games::add_game_from_exe,
        commands::games::update_game,
        commands::games::delete_game,
        commands::games::list_game_executables,
        commands::games::add_game_executable,
        commands::games::remove_game_executable,
        commands::games::pick_executable,
        commands::sessions::list_sessions,
        commands::sessions::create_session,
        commands::sessions::update_session,
        commands::sessions::delete_session,
        commands::stats::get_dashboard_stats,
        commands::stats::get_daily_totals,
        commands::stats::get_game_totals,
        commands::stats::get_hourly_totals,
        commands::settings::get_settings,
        commands::settings::update_settings,
        commands::dev::seed_demo_data,
        commands::imports::list_import_sources,
        commands::imports::scan_import_source,
        commands::imports::import_games,
        commands::imports::get_steam_api_settings,
        commands::imports::set_steam_api_settings,
        commands::imports::fetch_steam_playtime,
        commands::limits::list_limits,
        commands::limits::create_limit,
        commands::limits::update_limit,
        commands::limits::delete_limit,
        commands::library::list_tags,
        commands::library::create_tag,
        commands::library::rename_tag,
        commands::library::delete_tag,
        commands::library::set_game_tags,
        commands::library::merge_games,
        commands::library::pick_image,
        commands::library::set_game_cover,
        commands::data::export_data,
        commands::data::backup_database,
        commands::data::restore_database,
        commands::autostart::get_autostart,
        commands::autostart::set_autostart,
        commands::updates::check_for_update,
        commands::updates::install_update,
    ])
}

const BINDINGS_PATH: &str = "../src/bindings.ts";

pub fn export_bindings(builder: &tauri_specta::Builder<tauri::Wry>) {
    builder
        .export(specta_typescript::Typescript::default(), BINDINGS_PATH)
        .expect("failed to export TypeScript bindings");
}

fn init_logging(log_dir: &std::path::Path) -> anyhow::Result<LogGuard> {
    std::fs::create_dir_all(log_dir)?;
    let appender = tracing_appender::rolling::daily(log_dir, "gamelapse.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_ansi(false);
    let registry = tracing_subscriber::registry().with(filter).with(file_layer);
    if cfg!(debug_assertions) {
        registry
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
            .try_init()?;
    } else {
        registry.try_init()?;
    }
    Ok(LogGuard(guard))
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Gamelapse")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = specta_builder();
    #[cfg(debug_assertions)]
    export_bindings(&builder);

    tauri::Builder::default()
        // Must be registered first.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        // Without VISIBLE the plugin never shows the window, so `--minimized` stays hidden.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let data_dir = app.path().app_data_dir()?;
            let log_dir = app.path().app_log_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            let guard = init_logging(&log_dir)?;
            app.manage(guard);

            if let Some(roaming) = data_dir.parent() {
                let legacy_dir = roaming.join(core::migrate_legacy::LEGACY_IDENTIFIER);
                match core::migrate_legacy::migrate_legacy_data(&legacy_dir, &data_dir) {
                    Ok(true) => tracing::info!(
                        from = %legacy_dir.display(),
                        to = %data_dir.display(),
                        "migrated data from the GameTimeTracker folder"
                    ),
                    Ok(false) => {}
                    Err(e) => tracing::error!(error = %e, "legacy data migration failed"),
                }
            }

            let db_path = data_dir.join("gamelapse.db");
            tracing::info!(data_dir = %data_dir.display(), db_path = %db_path.display(), "starting Gamelapse");
            let conn = core::db::open(&db_path)?;
            match core::tracker::recover_crashed_sessions(&conn) {
                Ok(0) => {}
                Ok(n) => tracing::info!(closed = n, "closed sessions left open by a crash"),
                Err(e) => tracing::error!(error = %e, "crash recovery failed"),
            }

            let settings = commands::settings::load_settings(app.handle());
            app.manage(AppState {
                db: Mutex::new(conn),
                tracker: Mutex::new(core::tracker::Tracker::new()),
                settings: RwLock::new(settings),
                paused: AtomicBool::new(false),
                data_dir,
                db_path,
                log_dir,
            });

            setup_tray(app.handle())?;
            runtime::spawn_tracker(app.handle().clone());
            runtime::spawn_startup_import(app.handle().clone());
            commands::updates::spawn_startup_update_check(app.handle().clone());

            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window.label() == MAIN_WINDOW
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
