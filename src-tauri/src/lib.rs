mod commands;
mod db;
mod logger;
mod reminders;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};

use logger::log;

pub type Shared = Arc<Mutex<Connection>>;

pub struct AppState {
    db: Shared,
    pub data_dir: PathBuf,
    pub notice: Mutex<Option<String>>,
    pub close_to_tray: AtomicBool,
    pub start_hidden: AtomicBool,
    pub tray_ok: AtomicBool,
    pub started: std::time::Instant,
}

impl AppState {
    /// Mutex envenenado (panic em outra thread) não trava o app: recupera a conexão.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Pede para a UI salvar o que estiver pendente e encerrar. Se a UI não
/// responder em 1,5 s (travada, por exemplo), encerra mesmo assim.
fn request_quit(app: &AppHandle) {
    let _ = app.emit("request-quit", ());
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        handle.exit(0);
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Abrir TaskForMe", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("TaskForMe")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "open" => show_main(app),
            "quit" => request_quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
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
    let result = tauri::Builder::default()
        // Precisa ser o primeiro plugin: uma segunda instância só foca a janela
        // existente, evitando dois processos escrevendo no mesmo banco.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            let started = std::time::Instant::now();
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            logger::init(data_dir.clone());
            log(&format!("Iniciando TaskForMe {}", env!("CARGO_PKG_VERSION")));

            let opened = db::open(&data_dir).map_err(|e| {
                log(&format!("Falha fatal ao abrir banco: {e}"));
                e
            })?;

            let close_to_tray = opened
                .conn
                .query_row("SELECT value FROM settings WHERE key = 'closeToTray'", [], |r| r.get::<_, String>(0))
                .map(|v| v == "true")
                .unwrap_or(true);

            let shared: Shared = Arc::new(Mutex::new(opened.conn));
            let handle = app.handle().clone();

            let tray_ok = match build_tray(&handle) {
                Ok(()) => true,
                Err(e) => {
                    log(&format!("Bandeja indisponível: {e}"));
                    false
                }
            };
            let start_hidden = tray_ok && std::env::args().any(|a| a == "--minimized");

            app.manage(AppState {
                db: shared.clone(),
                data_dir: data_dir.clone(),
                notice: Mutex::new(opened.notice),
                close_to_tray: AtomicBool::new(close_to_tray),
                start_hidden: AtomicBool::new(start_hidden),
                tray_ok: AtomicBool::new(tray_ok),
                started,
            });

            reminders::spawn(handle.clone(), shared, data_dir);

            // Rede de segurança: se a UI não chamar `app_ready` (erro de carregamento),
            // a janela aparece mesmo assim em 4 s.
            if !start_hidden {
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(4));
                    if let Some(w) = handle.get_webview_window("main") {
                        if !w.is_visible().unwrap_or(true) {
                            show_main(&handle);
                        }
                    }
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                api.prevent_close();
                if state.tray_ok.load(Ordering::Relaxed) && state.close_to_tray.load(Ordering::Relaxed) {
                    let _ = window.hide();
                } else {
                    request_quit(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::app_ready,
            commands::quit_app,
            commands::setting_set,
            commands::autostart_get,
            commands::autostart_set,
            commands::backup_create,
            commands::open_data_dir,
            commands::projects_list,
            commands::project_create,
            commands::project_update,
            commands::project_move,
            commands::project_delete,
            commands::restore,
            commands::sections_list,
            commands::section_create,
            commands::section_update,
            commands::section_delete,
            commands::task_create,
            commands::task_update,
            commands::task_set_done,
            commands::task_delete,
            commands::task_get,
            commands::tasks_by_project,
            commands::subtasks,
            commands::note_create,
            commands::note_update,
            commands::note_delete,
            commands::note_get,
            commands::notes_by_project,
            commands::agenda,
            commands::counts,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        log(&format!("Erro fatal: {e}"));
        std::process::exit(1);
    }
}
