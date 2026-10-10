//! Verificador de lembretes. Roda numa thread própria enquanto o app estiver
//! aberto (inclusive minimizado na bandeja). Lembretes que venceram com o app
//! fechado disparam na próxima abertura.

use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use chrono::Local;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};
use tauri_plugin_notification::NotificationExt;

use crate::logger::log;
use crate::Shared;

const INTERVAL: Duration = Duration::from_secs(15);
/// A cada 240 rodadas (~1 h) verifica se o backup diário está em dia.
const BACKUP_EVERY: u32 = 240;

struct Due {
    title: String,
    body: String,
}

pub fn spawn(app: AppHandle, conn: Shared, data_dir: PathBuf) {
    let mut ticks: u32 = 0;
    thread::Builder::new()
        .name("reminders".into())
        .spawn(move || loop {
            ticks = ticks.wrapping_add(1);
            if ticks % BACKUP_EVERY == 0 {
                let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
                crate::db::backup_if_due(&conn, &data_dir);
            }
            // Um erro (ou até um panic) numa rodada nunca derruba a thread.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tick(&app, &conn)));
            match result {
                Ok(Err(e)) => log(&format!("Lembretes: {e}")),
                Err(_) => log("Lembretes: panic capturado"),
                _ => {}
            }
            thread::sleep(INTERVAL);
        })
        .map(|_| ())
        .unwrap_or_else(|e| log(&format!("Não foi possível iniciar lembretes: {e}")));
}

fn tick(app: &AppHandle, conn: &Shared) -> rusqlite::Result<()> {
    let due = {
        let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
        collect_due(&conn)?
    };
    if due.is_empty() {
        return Ok(());
    }
    for d in &due {
        if let Err(e) = app.notification().builder().title(&d.title).body(&d.body).show() {
            log(&format!("Falha ao mostrar notificação: {e}"));
        }
    }
    let _ = app.emit("reminders-fired", due.len());
    Ok(())
}

fn collect_due(conn: &Connection) -> rusqlite::Result<Vec<Due>> {
    let now = Local::now().format("%Y-%m-%dT%H:%M").to_string();
    let tx = conn.unchecked_transaction()?;
    let mut out = Vec::new();

    {
        let mut stmt = tx.prepare(
            "SELECT t.title, p.name, t.remind_at FROM tasks t JOIN projects p ON p.id = t.project_id
             WHERE t.reminded = 0 AND t.remind_at IS NOT NULL AND t.remind_at <= ?1
               AND t.done = 0 AND t.deleted_at IS NULL",
        )?;
        let rows = stmt.query_map([&now], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        for row in rows {
            let (title, project, at) = row?;
            out.push(Due { title: format!("⏰ {title}"), body: format!("{project} · {}", hhmm(&at)) });
        }
    }
    {
        let mut stmt = tx.prepare(
            "SELECT n.title, n.body, p.name FROM notes n JOIN projects p ON p.id = n.project_id
             WHERE n.reminded = 0 AND n.remind_at IS NOT NULL AND n.remind_at <= ?1 AND n.deleted_at IS NULL",
        )?;
        let rows = stmt.query_map([&now], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?)))?;
        for row in rows {
            let (title, body, project) = row?;
            let title = if title.trim().is_empty() { "Nota".to_string() } else { title };
            let preview: String = body.chars().take(120).collect();
            out.push(Due { title: format!("📝 {title}"), body: if preview.is_empty() { project } else { preview } });
        }
    }

    tx.execute(
        "UPDATE tasks SET reminded = 1 WHERE reminded = 0 AND remind_at IS NOT NULL AND remind_at <= ?1",
        [&now],
    )?;
    tx.execute(
        "UPDATE notes SET reminded = 1 WHERE reminded = 0 AND remind_at IS NOT NULL AND remind_at <= ?1",
        [&now],
    )?;
    tx.commit()?;
    Ok(out)
}

fn hhmm(at: &str) -> &str {
    at.get(11..16).unwrap_or(at)
}
