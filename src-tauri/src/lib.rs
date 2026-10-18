pub mod commands;
pub mod db;
pub mod feed;
pub mod fetch;
pub mod fulltext;
pub mod markdown;
pub mod opml;
pub mod proxy;
pub mod sanitize;
pub mod stockroom;
pub mod thumb;

use commands::*;
use tauri::Manager;

/// The log file's name in the app's log folder (without ".log").
pub const LOG_FILE: &str = "feedability";

/// Panics into the log too: release builds abort on panic, so stderr is all
/// that would be left, and nobody sees it.
fn log_panics() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("{info}");
        default(info);
    }));
}

/// The window, from tauri.conf.json (where it isn't created by itself), in
/// the saved theme: light or dark, or the OS's (docs/THEME.md).
fn main_window(app: &tauri::AppHandle) -> tauri::Result<()> {
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .expect("window in tauri.conf.json");
    let window = tauri::WebviewWindowBuilder::from_config(app, config)?.build()?;
    apply_theme(&window, saved_theme(app));
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The menu bar is made by the frontend from its command table
    // (src/lib/menuBar.ts); until it loads, macOS shows Tauri's default menu.
    tauri::Builder::default()
        // First, so a second launch quits before setting anything up: it only
        // brings this app's window up. Two apps on one database would refresh
        // the same feeds twice and wait on each other's writes.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app)
        }))
        // log::* from Rust and the frontend's warnings (src/lib/log.ts), into
        // the app's log folder ("ログを表示" in the Help menu) and stdout.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some(LOG_FILE.into()),
                    }),
                ])
                .max_file_size(1_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(2))
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Size, place and maximized of the window, kept between launches
        // (.window-state.json in the app config folder). Not fullscreen or
        // hidden: the app should always open as a visible, ordinary window.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .manage(AppState::default())
        .setup(|app| {
            log_panics();
            log::info!(
                "Feedability {} ({} {})",
                app.package_info().version,
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            // The database before the window: the page asks for the list
            // as soon as it loads. Without the database the window still
            // opens; every command then says so, and the log has why.
            if let Err(e) = open_db(app.handle()) {
                log::error!("database: {e}");
            }
            main_window(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_app_settings,
            set_app_settings,
            get_sidebar,
            query_articles,
            get_article,
            set_read,
            set_starred,
            set_later,
            mark_all_read,
            image_data_url,
            thumbnail_data_url,
            open_url,
            add_feed,
            get_feed_info,
            change_feed_url,
            rename_feed,
            move_feed,
            delete_feed,
            create_folder,
            rename_folder,
            delete_folder,
            refresh_feeds,
            cancel_refresh,
            feed_icons,
            set_feed_full_text,
            fetch_full_text,
            article_markdown,
            stockroom_status,
            stockroom_pair,
            stockroom_disconnect,
            stockroom_choices,
            set_stockroom_target,
            send_to_stockroom,
            import_opml,
            export_opml,
            reveal_log,
            system_proxy,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // Written back before quitting: feedability.db complete on its own.
            if let tauri::RunEvent::Exit = event {
                close_db(app);
            }
        });
}
