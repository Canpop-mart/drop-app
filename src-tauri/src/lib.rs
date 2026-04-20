#![deny(unused_must_use)]
#![feature(fn_traits)]
#![feature(duration_constructors)]
#![feature(duration_millis_float)]
#![feature(iterator_try_collect)]
#![feature(nonpoison_mutex)]
#![feature(sync_nonpoison)]
#![deny(clippy::all)]

use std::{
    env, fs::File, io::Write, panic::PanicHookInfo, path::Path, str::FromStr,
    sync::nonpoison::Mutex, time::SystemTime,
};

use ::client::{
    app_state::{AppState, SessionType, UmuState},
    app_status::AppStatus,
    autostart::sync_autostart_on_startup,
    compat::UMU_LAUNCHER_EXECUTABLE,
};
use ::download_manager::DownloadManagerWrapper;
use ::games::scan::scan_install_dirs;
use ::process::ProcessManagerWrapper;
use ::remote::{
    auth::{self, HandshakeRequestBody, HandshakeResponse, generate_authorization_header},
    cache::clear_cached_object,
    error::RemoteAccessError,
    fetch_object::fetch_object_wrapper,
    server_proto::handle_server_proto_wrapper,
    utils::{DROP_APP_HANDLE, DROP_CLIENT_ASYNC},
};
use database::{
    DB, GameDownloadStatus, borrow_db_checked, borrow_db_mut_checked, db::DATA_ROOT_DIR,
};
use log::{LevelFilter, debug, info, warn};
use log4rs::{
    Config,
    append::{console::ConsoleAppender, file::FileAppender},
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, RunEvent, WebviewBuilder, WebviewUrl,
    WindowBuilder, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_dialog::DialogExt;
use url::Url;
use utils::app_emit;

mod client;
mod collections;
mod download_manager;
mod downloads;
mod games;
mod process;
mod remote;
mod scheduler;
mod settings;
mod streaming;
mod updates;

use client::*;
use download_manager::*;
use downloads::*;
use games::*;
use process::*;
use remote::*;
use settings::*;
use streaming::*;

use crate::scheduler::scheduler_task;

async fn setup(handle: AppHandle) -> AppState {
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d} | {l} | {f}:{L} - {m}{n}",
        )))
        .append(false)
        .build(DATA_ROOT_DIR.join("./drop.log"))
        .expect("Failed to setup logfile");

    let console = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(
            "{d} | {h({l})} | {f}:{L} - {m}{n}",
        )))
        .build();

    let log_level = env::var("RUST_LOG").unwrap_or(String::from("Info"));

    let config = Config::builder()
        .appenders(vec![
            Appender::builder().build("logfile", Box::new(logfile)),
            Appender::builder().build("console", Box::new(console)),
        ])
        .build(
            Root::builder()
                .appenders(vec!["logfile", "console"])
                .build(LevelFilter::from_str(&log_level).expect("Invalid log level")),
        )
        .expect("Failed to build config");

    log4rs::init_config(config).expect("Failed to initialise log4rs");

    ProcessManagerWrapper::init(handle.clone());
    DownloadManagerWrapper::init(handle.clone());

    // Gamepad input is now handled via the Web Gamepad API in the Vue
    // frontend (composables/gamepad.ts) because gilrs's WGI backend on
    // Windows intermittently fails to deliver input for HID controllers.
    // ::process::gamepad::start_polling(handle.clone());

    debug!("checking if database is set up");
    let is_set_up = DB.database_is_set_up();

    #[cfg(not(target_os = "linux"))]
    let umu_state = UmuState::NotNeeded;

    #[cfg(target_os = "linux")]
    let umu_state = match UMU_LAUNCHER_EXECUTABLE.is_some() {
        true => UmuState::Installed,
        false => UmuState::NotInstalled,
    };

    let session_type = SessionType::detect();
    info!("detected session type: {:?}", session_type);

    scan_install_dirs();

    if !is_set_up {
        return AppState {
            status: AppStatus::NotConfigured,
            user: None,
            umu_state,
            session_type,
        };
    }

    debug!("database is set up");

    let (app_status, user) = auth::setup().await;

    let missing_games = {
        let db_handle = borrow_db_checked();
        db_handle
            .applications
            .game_statuses
            .iter()
            .filter_map(|(game_id, status)| match status {
                GameDownloadStatus::Remote {} => None,
                GameDownloadStatus::Installed { install_dir, .. } => {
                    if !Path::new(install_dir).exists() {
                        Some(game_id.clone())
                    } else {
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
    };

    info!("detected games missing: {missing_games:?}");

    let mut db_handle = borrow_db_mut_checked();
    for game_id in missing_games {
        db_handle
            .applications
            .game_statuses
            .entry(game_id)
            .and_modify(|v| *v = GameDownloadStatus::Remote {});
    }

    drop(db_handle);

    debug!("finished setup!");

    // Sync autostart state
    if let Err(e) = sync_autostart_on_startup(&handle) {
        warn!("failed to sync autostart state: {e}");
    }

    AppState {
        status: app_status,
        user,
        umu_state,
        session_type,
    }
}

pub fn custom_panic_handler(e: &PanicHookInfo) -> Option<()> {
    let crash_file = DATA_ROOT_DIR.join(format!(
        "crash-{}.log",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs()
    ));
    let mut file = File::create_new(crash_file).ok()?;
    file.write_all(format!("Drop crashed with the following panic:\n{e}").as_bytes())
        .ok()?;
    drop(file);

    Some(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // let global_span = span!(Level::TRACE, "global_span");
    // let _enter = global_span.enter();

    // Workaround for WebKitGTK EGL crash on SteamOS / non-standard GPU drivers.
    // The bundled WebKitGTK in AppImages can fail with "Could not create default
    // EGL display: EGL_BAD_PARAMETER" — disabling the DMA-BUF renderer forces a
    // fallback that works on Steam Deck and other embedded Linux systems.
    // SAFETY: Called at the very start of main, before any threads are spawned,
    // so there are no concurrent readers of the environment.
    #[cfg(target_os = "linux")]
    {
        if env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
            unsafe { env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
        }
    }

    std::panic::set_hook(Box::new(|e| {
        let _ = custom_panic_handler(e);
        println!("{e}");
    }));

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(desktop)]
    #[allow(unused_variables)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|_app, argv, _cwd| {
            // when defining deep link schemes at runtime, you must also check `argv` here
        }));
    }

    let app = builder
        .plugin(tauri_plugin_deep_link::init())
        .invoke_handler(tauri::generate_handler![
            // Core utils
            fetch_state,
            quit,
            fetch_system_data,
            collect_bug_report_diagnostics,
            collect_bug_report_logs,
            collect_bug_report_log_file,
            open_log_folder,
            open_steam_keyboard,
            open_fs,
            // User utils
            update_settings,
            fetch_settings,
            // Auth
            auth_initiate,
            auth_initiate_code,
            retry_connect,
            manual_recieve_handshake,
            sign_out,
            // Remote
            use_remote,
            gen_drop_url,
            fetch_drop_object,
            check_online,
            // Library
            fetch_library,
            fetch_game,
            add_download_dir,
            delete_download_dir,
            fetch_download_dir_stats,
            fetch_game_status,
            fetch_game_statuses,
            fetch_game_version_options,
            update_game_configuration,
            configure_game_emulator,
            get_install_size,
            list_game_saves,
            delete_game_save,
            read_save_file,
            write_save_file,
            read_pc_save_file,
            write_pc_save_file,
            list_pc_game_saves,
            backup_pc_game_saves,
            restore_pc_game_saves,
            check_ludusavi,
            install_ludusavi,
            check_ra_rom_hash,
            // Downloads
            download_game,
            resume_download,
            move_download_in_queue,
            pause_downloads,
            resume_downloads,
            cancel_game,
            uninstall_game,
            // Processes
            launch_game,
            kill_game,
            resolve_save_conflicts,
            toggle_autostart,
            get_autostart_enabled,
            open_process_logs,
            read_latest_launch_log,
            get_launch_options,
            detect_package_format,
            detect_removable_storage,
            #[cfg(target_os = "linux")]
            ::process::compat::fetch_proton_paths,
            #[cfg(target_os = "linux")]
            ::process::compat::add_proton_layer,
            #[cfg(target_os = "linux")]
            ::process::compat::remove_proton_layer,
            #[cfg(target_os = "linux")]
            ::process::compat::set_default,
            #[cfg(target_os = "linux")]
            ::process::compat::clear_default_proton,
            #[cfg(target_os = "linux")]
            ::process::compat::diagnose_launch_environment,
            #[cfg(target_os = "linux")]
            ::process::compat::install_umu,
            // Streaming (Sunshine)
            check_sunshine,
            install_sunshine,
            sunshine_status,
            start_sunshine,
            stop_sunshine,
            sunshine_send_pin,
            sunshine_list_apps,
            sunshine_register_game,
            sunshine_list_clients,
            // Streaming sessions (server-side, JWT auth)
            streaming_create_session,
            streaming_mark_ready,
            streaming_stop_session,
            stop_all_host_sessions,
            streaming_heartbeat,
            streaming_list_sessions,
            streaming_get_connection_info,
            launch_moonlight,
            kill_moonlight,
            watch_moonlight_session,
            streaming_request_stream,
            list_devices,
            request_remote_install,
            sync_installed_games
        ])
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimize"]),
        ))
        .setup(|app| {
            let handle = app.handle().clone();

            tauri::async_runtime::block_on(async move {
                let state = setup(handle.clone()).await;
                info!("initialized drop client");

                // Start background poller for incoming stream requests
                streaming::spawn_stream_request_poller();

                // Sync installed games to server (fire and forget, only if authenticated)
                tokio::spawn(async {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    // Skip if not logged in (e.g. fresh install / DB cleared)
                    {
                        let db = borrow_db_checked();
                        if db.auth.is_none() || db.base_url.is_empty() {
                            return;
                        }
                    }
                    if let Err(e) = streaming::sync_installed_games().await {
                        log::warn!("Failed to sync installed games on startup: {e}");
                    }
                });

                let is_gamescope = state.session_type == SessionType::Gamescope;
                let is_deck_hw = state.session_type.is_steam_deck_hardware();
                app.manage(Mutex::new(state));

                let global_app_handle = handle;
                {
                    let mut app_handle_lock = DROP_APP_HANDLE.lock().await;
                    app_handle_lock.replace(global_app_handle);
                };

                {
                    use tauri_plugin_deep_link::DeepLinkExt;
                    let _ = app.deep_link().register_all();
                    debug!("registered all pre-defined deep links");
                }

                let handle = app.handle().clone();

                // In Gamescope on Deck hardware, primary_monitor() can
                // report the compositor's output resolution (1080p/4K when
                // docked) rather than the internal 1280x800 render target
                // gamescope actually hands to the client, which leaves the
                // webview rendering at the wrong size. Force native Deck
                // dimensions in that case. For non-Deck handhelds under
                // gamescope, keep the monitor query.
                let (width, height) = if is_gamescope {
                    if is_deck_hw {
                        info!("[STARTUP] Steam Deck hardware under gamescope — forcing 1280x800");
                        (1280.0, 800.0)
                    } else {
                        match handle.primary_monitor() {
                            Ok(Some(monitor)) => {
                                let size = monitor.size();
                                let scale = monitor.scale_factor().max(f64::EPSILON);
                                let logical_w = size.width as f64 / scale;
                                let logical_h = size.height as f64 / scale;
                                info!(
                                    "[STARTUP] Gamescope monitor: physical {}x{} @ scale {:.2} \
                                     → logical {:.0}x{:.0}",
                                    size.width, size.height, scale, logical_w, logical_h
                                );
                                (logical_w, logical_h)
                            }
                            other => {
                                warn!(
                                    "[STARTUP] Gamescope: couldn't read primary monitor ({other:?}), \
                                     falling back to 1280x800"
                                );
                                (1280.0, 800.0)
                            }
                        }
                    }
                } else {
                    (1536.0, 864.0)
                };

                let mut window_builder = WindowBuilder::new(&handle, "main")
                    .title("Drop Desktop App")
                    .min_inner_size(1000.0, 500.0)
                    .inner_size(width, height)
                    .resizable(true);

                // In Gamescope, go fullscreen and skip window decorations/shadow
                // (Gamescope handles the compositor chrome)
                if is_gamescope {
                    window_builder = window_builder
                        .fullscreen(true)
                        .decorations(false)
                        .shadow(false);
                } else {
                    window_builder = window_builder
                        .decorations(false)
                        .shadow(true);
                }

                let main_window = window_builder
                    .build()
                    .expect("failed to build main window");

                // On Gamescope, the fullscreen window ends up at the compositor
                // resolution (which may differ from what we passed as
                // inner_size because the compositor enforces its own size).
                // Read the window's actual inner size back so the webview
                // starts at the real display size — otherwise `.auto_resize()`
                // has to chase the window on first paint and the UI renders at
                // the wrong resolution for the first frame (and sometimes
                // permanently, on Wayland compositors that batch resize events
                // before the webview attaches).
                let (webview_w, webview_h) = if is_gamescope {
                    match main_window.inner_size() {
                        Ok(phys) => {
                            let scale = main_window.scale_factor().unwrap_or(1.0).max(f64::EPSILON);
                            let w = phys.width as f64 / scale;
                            let h = phys.height as f64 / scale;
                            info!(
                                "[STARTUP] Gamescope window actual size: physical {}x{} @ scale {:.2} \
                                 → logical {:.0}x{:.0}",
                                phys.width, phys.height, scale, w, h
                            );
                            (w, h)
                        }
                        Err(e) => {
                            warn!("[STARTUP] Couldn't read window inner size ({e}), using requested {width}x{height}");
                            (width, height)
                        }
                    }
                } else {
                    (width, height)
                };

                main_window
                    .add_child(
                        WebviewBuilder::new("frontend", WebviewUrl::App("main".into()))
                            .auto_resize(),
                        LogicalPosition::new(0., 0.),
                        LogicalSize::new(webview_w, webview_h),
                    )
                    .expect("failed to create frontend webview");

                app.deep_link().on_open_url(move |event| {
                    debug!("handling drop:// url");
                    let binding = event.urls();
                    let url = match binding.first() {
                        Some(url) => url,
                        None => {
                            warn!("No value recieved from deep link. Is this a drop server?");
                            return;
                        }
                    };
                    if let Some("handshake") = url.host_str() {
                        let path = url.path().to_string();
                        tauri::async_runtime::spawn(recieve_handshake(
                            handle.clone(),
                            path,
                        ));
                    }
                });
                let open_menu_item = MenuItem::with_id(app, "open", "Open", true, None::<&str>)
                    .expect("Failed to generate open menu item");

                let sep = PredefinedMenuItem::separator(app)
                    .expect("Failed to generate menu separator item");

                let quit_menu_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
                    .expect("Failed to generate quit menu item");

                let menu = Menu::with_items(
                    app,
                    &[
                        &open_menu_item,
                        &sep,
                        &quit_menu_item,
                    ],
                )
                .expect("Failed to generate menu");

                run_on_tray(|| {
                    TrayIconBuilder::new()
                        .icon(
                            app.default_window_icon()
                                .expect("Failed to get default window icon")
                                .clone(),
                        )
                        .menu(&menu)
                        .on_menu_event(|app, event| match event.id.as_ref() {
                            "open" => {
                                app.webview_windows()
                                    .get("frontend")
                                    .expect("Failed to get webview")
                                    .show()
                                    .expect("Failed to show window");
                            }
                            "quit" => {
                                app.exit(0);
                            }

                            _ => {
                                warn!("menu event not handled: {:?}", event.id);
                            }
                        })
                        .build(app)
                        .expect("error while setting up tray menu");
                });

                {
                    let mut db_handle = borrow_db_mut_checked();
                    if let Some(original) = db_handle.prev_database.take() {
                        let canonicalised = match original.canonicalize() {
                            Ok(o) => o,
                            Err(_) => original,
                        };
                        warn!(
                            "Database corrupted. Original file at {}",
                            canonicalised.display()
                        );
                        app.dialog()
                            .message(format!(
                                "Database corrupted. A copy has been saved at: {}",
                                canonicalised.display()
                            ))
                            .title("Database corrupted")
                            .show(|_| {});
                    }
                }

                tokio::spawn(async move { scheduler_task().await });
            });

            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("object", move |_ctx, request, responder| {
            tauri::async_runtime::spawn(async move {
                fetch_object_wrapper(request, responder).await;
            });
        })
        .register_asynchronous_uri_scheme_protocol("server", |_ctx, request, responder| {
            tauri::async_runtime::spawn(async move {
                handle_server_proto_wrapper(request, responder).await;
            });
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                run_on_tray(|| {
                    window.hide().expect("Failed to close window in tray");
                    api.prevent_close();
                });
            }
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|_app_handle, event| {
        if let RunEvent::ExitRequested { code, api, .. } = event {
            run_on_tray(|| {
                if code.is_none() {
                    api.prevent_exit();
                }
            });
        }
    });
}

fn run_on_tray<T: FnOnce()>(f: T) {
    if match std::env::var("NO_TRAY_ICON") {
        Ok(s) => s.to_lowercase() != "true",
        Err(_) => true,
    } {
        (f)();
    }
}

// TODO: Refactor
pub async fn recieve_handshake(app: AppHandle, path: String) {
    // Tell the app we're processing
    app_emit!(&app, "auth/processing", ());

    let handshake_result = recieve_handshake_logic(&app, path).await;
    if let Err(e) = handshake_result {
        warn!("error with authentication: {e}");
        app_emit!(&app, "auth/failed", e.to_string());
        return;
    }

    let app_state = app.state::<Mutex<AppState>>();

    let (app_status, user) = auth::setup().await;

    let mut state_lock = app_state.lock();

    state_lock.status = app_status;
    state_lock.user = user;

    let _ = clear_cached_object("collections");
    let _ = clear_cached_object("library");

    drop(state_lock);

    app_emit!(&app, "auth/finished", ());
}

// TODO: Refactor
async fn recieve_handshake_logic(app: &AppHandle, path: String) -> Result<(), RemoteAccessError> {
    let path_chunks: Vec<&str> = path.split('/').collect();
    if path_chunks.len() != 3 {
        app_emit!(app, "auth/failed", ());
        return Err(RemoteAccessError::HandshakeFailed(
            "failed to parse token".to_string(),
        ));
    }

    let base_url = {
        let handle = borrow_db_checked();
        Url::parse(handle.base_url.as_str())?
    };

    let client_id = path_chunks.get(1).ok_or_else(|| {
        RemoteAccessError::HandshakeFailed("missing client id in handshake path".to_string())
    })?;
    let token = path_chunks.get(2).ok_or_else(|| {
        RemoteAccessError::HandshakeFailed("missing token in handshake path".to_string())
    })?;
    // Client id and token are opaque identifiers issued by the server. Reject
    // anything with structural characters so a malformed deep link can't be
    // coerced into something weirder downstream.
    let is_token_char = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
    if client_id.is_empty() || !client_id.chars().all(is_token_char) {
        return Err(RemoteAccessError::HandshakeFailed(
            "invalid client id in handshake path".to_string(),
        ));
    }
    if token.is_empty() || !token.chars().all(is_token_char) {
        return Err(RemoteAccessError::HandshakeFailed(
            "invalid token in handshake path".to_string(),
        ));
    }
    let body = HandshakeRequestBody::new((*client_id).to_string(), (*token).to_string());

    let endpoint = base_url.join("/api/v1/client/auth/handshake")?;
    let client = DROP_CLIENT_ASYNC.clone();
    let response = client.post(endpoint).json(&body).send().await?;
    debug!("handshake responsded with {}", response.status().as_u16());
    if !response.status().is_success() {
        return Err(RemoteAccessError::InvalidResponse(response.json().await?));
    }
    let response_struct: HandshakeResponse = response.json().await?;

    {
        let mut handle = borrow_db_mut_checked();
        handle.auth = Some(response_struct.into());
    }

    let web_token = {
        let header = generate_authorization_header();
        let token = client
            .post(base_url.join("/api/v1/client/user/webtoken")?)
            .header("Authorization", header)
            .send()
            .await?;

        token.text().await?
    };
    let mut handle = borrow_db_mut_checked();
    if let Some(auth) = handle.auth.as_mut() {
        auth.web_token = Some(web_token);
    } else {
        warn!("Auth not initialized when setting web token");
    }

    Ok(())
}
