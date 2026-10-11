//! Comandos chamados pela interface. Toda entrada é validada aqui: a UI nunca
//! consegue gravar data inválida, referência quebrada ou texto vazio.

use std::sync::atomic::Ordering;

use chrono::{Local, NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_opener::OpenerExt;

use crate::logger::log;
use crate::AppState;

// ---------------------------------------------------------------- erros

#[derive(Debug, Serialize)]
pub struct AppError(String);

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        log(&format!("Erro SQLite: {e}"));
        AppError(format!("Erro no banco de dados: {e}"))
    }
}

impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError(s)
    }
}

fn fail<T>(msg: &str) -> Result<T, AppError> {
    Err(AppError(msg.to_string()))
}

type Res<T> = Result<T, AppError>;

// ---------------------------------------------------------------- helpers

fn now() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Token único de exclusão: todas as linhas apagadas juntas recebem o mesmo
/// valor em `deleted_at`, o que permite desfazer a operação inteira.
fn delete_token() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S%.6f").to_string()
}

fn clean_text(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control() || *c == '\n' || *c == '\t').take(max).collect()
}

fn clean_title(s: &str, fallback: &str) -> String {
    let t = clean_text(s.trim(), 500).replace(['\n', '\t'], " ");
    if t.trim().is_empty() { fallback.to_string() } else { t.trim().to_string() }
}

fn valid_date(d: Option<String>) -> Res<Option<String>> {
    match d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => NaiveDate::parse_from_str(&s, "%Y-%m-%d")
            .map(|d| Some(d.format("%Y-%m-%d").to_string()))
            .or_else(|_| fail("Data inválida")),
    }
}

fn valid_datetime(d: Option<String>) -> Res<Option<String>> {
    match d.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M")
            .map(|d| Some(d.format("%Y-%m-%dT%H:%M").to_string()))
            .or_else(|_| fail("Data/hora do lembrete inválida")),
    }
}

fn valid_color(c: &str) -> String {
    let ok = c.len() == 7
        && c.starts_with('#')
        && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if ok { c.to_lowercase() } else { "#5e6ad2".into() }
}

fn ensure_project(conn: &Connection, id: i64) -> Res<()> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL)",
        [id],
        |r| r.get(0),
    )?;
    if exists { Ok(()) } else { fail("Projeto não encontrado") }
}

/// Retorna a seção só se ela pertencer ao projeto; senão, nenhuma.
fn section_in_project(conn: &Connection, section: Option<i64>, project: i64) -> Res<Option<i64>> {
    let Some(sid) = section else { return Ok(None) };
    let ok: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sections WHERE id = ?1 AND project_id = ?2 AND deleted_at IS NULL)",
        params![sid, project],
        |r| r.get(0),
    )?;
    Ok(ok.then_some(sid))
}

fn inbox_id(conn: &Connection) -> Res<i64> {
    Ok(conn.query_row(
        "SELECT id FROM projects WHERE is_inbox = 1 ORDER BY id LIMIT 1",
        [],
        |r| r.get(0),
    )?)
}

// ---------------------------------------------------------------- modelos

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    id: i64,
    name: String,
    color: String,
    is_inbox: bool,
    position: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    id: i64,
    project_id: i64,
    name: String,
    position: i64,
    collapsed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    id: i64,
    project_id: i64,
    section_id: Option<i64>,
    parent_id: Option<i64>,
    title: String,
    description: String,
    due_date: Option<String>,
    remind_at: Option<String>,
    done: bool,
    position: i64,
    created_at: String,
    project_name: String,
    project_color: String,
    parent_title: Option<String>,
}

const TASK_SELECT: &str = "
    SELECT t.id, t.project_id, t.section_id, t.parent_id, t.title, t.description,
           t.due_date, t.remind_at, t.done, t.position, t.created_at,
           p.name, p.color, par.title
    FROM tasks t
    JOIN projects p    ON p.id = t.project_id
    LEFT JOIN tasks par ON par.id = t.parent_id
    WHERE t.deleted_at IS NULL";

fn task_from(r: &Row) -> rusqlite::Result<Task> {
    Ok(Task {
        id: r.get(0)?,
        project_id: r.get(1)?,
        section_id: r.get(2)?,
        parent_id: r.get(3)?,
        title: r.get(4)?,
        description: r.get(5)?,
        due_date: r.get(6)?,
        remind_at: r.get(7)?,
        done: r.get(8)?,
        position: r.get(9)?,
        created_at: r.get(10)?,
        project_name: r.get(11)?,
        project_color: r.get(12)?,
        parent_title: r.get(13)?,
    })
}

fn get_task(conn: &Connection, id: i64) -> Res<Task> {
    conn.query_row(&format!("{TASK_SELECT} AND t.id = ?1"), [id], task_from)
        .optional()?
        .ok_or_else(|| AppError("Tarefa não encontrada".into()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    id: i64,
    project_id: i64,
    section_id: Option<i64>,
    title: String,
    body: String,
    note_date: Option<String>,
    remind_at: Option<String>,
    created_at: String,
    updated_at: String,
    project_name: String,
    project_color: String,
}

const NOTE_SELECT: &str = "
    SELECT n.id, n.project_id, n.section_id, n.title, n.body, n.note_date,
           n.remind_at, n.created_at, n.updated_at, p.name, p.color
    FROM notes n
    JOIN projects p ON p.id = n.project_id
    WHERE n.deleted_at IS NULL";

fn note_from(r: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get(0)?,
        project_id: r.get(1)?,
        section_id: r.get(2)?,
        title: r.get(3)?,
        body: r.get(4)?,
        note_date: r.get(5)?,
        remind_at: r.get(6)?,
        created_at: r.get(7)?,
        updated_at: r.get(8)?,
        project_name: r.get(9)?,
        project_color: r.get(10)?,
    })
}

fn get_note(conn: &Connection, id: i64) -> Res<Note> {
    conn.query_row(&format!("{NOTE_SELECT} AND n.id = ?1"), [id], note_from)
        .optional()?
        .ok_or_else(|| AppError("Nota não encontrada".into()))
}

// ---------------------------------------------------------------- bootstrap

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    projects: Vec<Project>,
    sections: Vec<Section>,
    settings: Vec<(String, String)>,
    notice: Option<String>,
    data_dir: String,
}

#[tauri::command]
pub fn bootstrap(state: State<AppState>) -> Res<Bootstrap> {
    let conn = state.conn();
    let settings = conn
        .prepare("SELECT key, value FROM settings")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Bootstrap {
        projects: list_projects(&conn)?,
        sections: list_sections(&conn)?,
        settings,
        notice: state.notice.lock().unwrap_or_else(|e| e.into_inner()).take(),
        data_dir: state.data_dir.display().to_string(),
    })
}

/// A janela começa escondida e só aparece quando a UI já renderizou:
/// evita o "flash" branco ao abrir.
#[tauri::command]
pub fn app_ready(app: AppHandle, state: State<AppState>) {
    log(&format!("UI pronta em {} ms", state.started.elapsed().as_millis()));
    if !state.start_hidden.load(Ordering::Relaxed) {
        crate::show_main(&app);
    }
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------- settings

#[tauri::command]
pub fn setting_set(state: State<AppState>, key: String, value: String) -> Res<()> {
    let allowed = ["theme", "closeToTray", "showDone"];
    if !allowed.contains(&key.as_str()) {
        return fail("Configuração desconhecida");
    }
    let value = clean_text(&value, 100);
    if key == "closeToTray" {
        state.close_to_tray.store(value == "true", Ordering::Relaxed);
    }
    state.conn().execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[tauri::command]
pub fn autostart_get(app: AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> Res<()> {
    let r = if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() };
    r.map_err(|e| AppError(format!("Não foi possível alterar a inicialização automática: {e}")))
}

#[tauri::command]
pub fn backup_create(state: State<AppState>) -> Res<String> {
    let conn = state.conn();
    let path = crate::db::backup_now(&conn, &state.data_dir, "manual")?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn open_data_dir(state: State<AppState>) -> Res<()> {
    let dir = state.data_dir.clone();
    #[cfg(target_os = "windows")]
    let cmd = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(&dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = std::process::Command::new("xdg-open").arg(&dir).spawn();
    cmd.map(|_| ()).map_err(|e| AppError(format!("Não foi possível abrir a pasta: {e}")))
}

// ---------------------------------------------------------------- projetos

fn list_projects(conn: &Connection) -> Res<Vec<Project>> {
    Ok(conn
        .prepare(
            "SELECT id, name, color, is_inbox, position FROM projects
             WHERE deleted_at IS NULL ORDER BY is_inbox DESC, position, id",
        )?
        .query_map([], |r| {
            Ok(Project {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                is_inbox: r.get(3)?,
                position: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn projects_list(state: State<AppState>) -> Res<Vec<Project>> {
    list_projects(&state.conn())
}

#[tauri::command]
pub fn project_create(state: State<AppState>, name: String, color: String) -> Res<i64> {
    let conn = state.conn();
    conn.execute(
        "INSERT INTO projects (name, color, position, created_at)
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM projects), ?3)",
        params![clean_title(&name, "Novo projeto"), valid_color(&color), now()],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn project_update(state: State<AppState>, id: i64, name: String, color: String) -> Res<()> {
    let conn = state.conn();
    ensure_project(&conn, id)?;
    conn.execute(
        "UPDATE projects SET name = ?2, color = ?3 WHERE id = ?1",
        params![id, clean_title(&name, "Projeto"), valid_color(&color)],
    )?;
    Ok(())
}

#[tauri::command]
pub fn project_move(state: State<AppState>, ids: Vec<i64>) -> Res<()> {
    let mut conn = state.conn();
    let tx = conn.transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute("UPDATE projects SET position = ?2 WHERE id = ?1 AND is_inbox = 0", params![id, i as i64 + 1])?;
    }
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn project_delete(state: State<AppState>, id: i64) -> Res<String> {
    let mut conn = state.conn();
    let is_inbox: Option<bool> = conn
        .query_row("SELECT is_inbox FROM projects WHERE id = ?1 AND deleted_at IS NULL", [id], |r| r.get(0))
        .optional()?;
    match is_inbox {
        None => return fail("Projeto não encontrado"),
        Some(true) => return fail("A Entrada não pode ser excluída"),
        Some(false) => {}
    }
    let token = delete_token();
    let tx = conn.transaction()?;
    for sql in [
        "UPDATE tasks    SET deleted_at = ?2 WHERE project_id = ?1 AND deleted_at IS NULL",
        "UPDATE notes    SET deleted_at = ?2 WHERE project_id = ?1 AND deleted_at IS NULL",
        "UPDATE links    SET deleted_at = ?2 WHERE project_id = ?1 AND deleted_at IS NULL",
        "UPDATE events   SET deleted_at = ?2 WHERE project_id = ?1 AND deleted_at IS NULL",
        "UPDATE sections SET deleted_at = ?2 WHERE project_id = ?1 AND deleted_at IS NULL",
        "UPDATE projects SET deleted_at = ?2 WHERE id = ?1",
    ] {
        tx.execute(sql, params![id, token])?;
    }
    tx.commit()?;
    Ok(token)
}

/// Desfaz qualquer exclusão feita com o token informado.
#[tauri::command]
pub fn restore(state: State<AppState>, token: String) -> Res<()> {
    log(&format!("restore {token}"));
    let mut conn = state.conn();
    let tx = conn.transaction()?;
    let mut restored = 0;
    for table in ["projects", "sections", "tasks", "notes", "links", "events"] {
        restored += tx.execute(&format!("UPDATE {table} SET deleted_at = NULL WHERE deleted_at = ?1"), [&token])?;
    }
    tx.commit()?;
    if restored == 0 {
        return fail("Não há mais nada para desfazer");
    }
    Ok(())
}

// ---------------------------------------------------------------- seções

fn list_sections(conn: &Connection) -> Res<Vec<Section>> {
    Ok(conn
        .prepare(
            "SELECT id, project_id, name, position, collapsed FROM sections
             WHERE deleted_at IS NULL ORDER BY position, id",
        )?
        .query_map([], |r| {
            Ok(Section {
                id: r.get(0)?,
                project_id: r.get(1)?,
                name: r.get(2)?,
                position: r.get(3)?,
                collapsed: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
}

#[tauri::command]
pub fn sections_list(state: State<AppState>) -> Res<Vec<Section>> {
    list_sections(&state.conn())
}

#[tauri::command]
pub fn section_create(state: State<AppState>, project_id: i64, name: String) -> Res<i64> {
    let conn = state.conn();
    ensure_project(&conn, project_id)?;
    conn.execute(
        "INSERT INTO sections (project_id, name, position)
         VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM sections WHERE project_id = ?1))",
        params![project_id, clean_title(&name, "Nova divisão")],
    )?;
    Ok(conn.last_insert_rowid())
}

#[tauri::command]
pub fn section_update(state: State<AppState>, id: i64, name: String, collapsed: bool) -> Res<()> {
    state.conn().execute(
        "UPDATE sections SET name = ?2, collapsed = ?3 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, clean_title(&name, "Divisão"), collapsed],
    )?;
    Ok(())
}

#[tauri::command]
pub fn section_delete(state: State<AppState>, id: i64) -> Res<String> {
    let mut conn = state.conn();
    let token = delete_token();
    let tx = conn.transaction()?;
    for sql in [
        "UPDATE tasks    SET deleted_at = ?2 WHERE section_id = ?1 AND deleted_at IS NULL",
        "UPDATE notes    SET deleted_at = ?2 WHERE section_id = ?1 AND deleted_at IS NULL",
        "UPDATE links    SET deleted_at = ?2 WHERE section_id = ?1 AND deleted_at IS NULL",
        "UPDATE sections SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
    ] {
        tx.execute(sql, params![id, token])?;
    }
    tx.commit()?;
    Ok(token)
}

// ---------------------------------------------------------------- tarefas

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    title: String,
    project_id: Option<i64>,
    section_id: Option<i64>,
    parent_id: Option<i64>,
    due_date: Option<String>,
}

#[tauri::command]
pub fn task_create(state: State<AppState>, input: NewTask) -> Res<Task> {
    let conn = state.conn();
    let due = valid_date(input.due_date)?;

    // Subtarefa herda projeto e seção do pai. Só existe um nível de subtarefa.
    let (project, section, parent) = match input.parent_id {
        Some(pid) => {
            let parent = get_task(&conn, pid)?;
            if parent.parent_id.is_some() {
                return fail("Subtarefas não podem ter subtarefas");
            }
            (parent.project_id, parent.section_id, Some(pid))
        }
        None => {
            let project = match input.project_id {
                Some(p) => { ensure_project(&conn, p)?; p }
                None => inbox_id(&conn)?,
            };
            (project, section_in_project(&conn, input.section_id, project)?, None)
        }
    };

    let ts = now();
    conn.execute(
        "INSERT INTO tasks (project_id, section_id, parent_id, title, due_date, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5,
                 (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE project_id = ?1),
                 ?6, ?6)",
        params![project, section, parent, clean_title(&input.title, "Nova tarefa"), due, ts],
    )?;
    get_task(&conn, conn.last_insert_rowid())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPatch {
    id: i64,
    title: String,
    description: String,
    due_date: Option<String>,
    remind_at: Option<String>,
    project_id: i64,
    section_id: Option<i64>,
}

#[tauri::command]
pub fn task_update(state: State<AppState>, input: TaskPatch) -> Res<Task> {
    let mut conn = state.conn();
    let current = get_task(&conn, input.id)?;
    let due = valid_date(input.due_date)?;
    let remind = valid_datetime(input.remind_at)?;

    let (project, section) = if current.parent_id.is_some() {
        (current.project_id, current.section_id)
    } else {
        ensure_project(&conn, input.project_id)?;
        (input.project_id, section_in_project(&conn, input.section_id, input.project_id)?)
    };

    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE tasks SET title = ?2, description = ?3, due_date = ?4, remind_at = ?5,
                reminded = CASE WHEN remind_at IS ?5 THEN reminded ELSE 0 END,
                project_id = ?6, section_id = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            input.id,
            clean_title(&input.title, "Sem título"),
            clean_text(&input.description, 20_000),
            due,
            remind,
            project,
            section,
            now()
        ],
    )?;
    // Ao mover a tarefa-mãe, as subtarefas vão junto.
    tx.execute(
        "UPDATE tasks SET project_id = ?2, section_id = ?3 WHERE parent_id = ?1",
        params![input.id, project, section],
    )?;
    tx.commit()?;
    get_task(&conn, input.id)
}

#[tauri::command]
pub fn task_set_done(state: State<AppState>, id: i64, done: bool) -> Res<()> {
    let mut conn = state.conn();
    let ts = now();
    let done_at = done.then(|| ts.clone());
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE tasks SET done = ?2, done_at = ?3, updated_at = ?4 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, done, done_at, ts],
    )?;
    // Concluir a tarefa-mãe conclui as subtarefas.
    if done {
        tx.execute(
            "UPDATE tasks SET done = 1, done_at = ?2, updated_at = ?2
             WHERE parent_id = ?1 AND done = 0 AND deleted_at IS NULL",
            params![id, ts],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn task_delete(state: State<AppState>, id: i64) -> Res<String> {
    let token = delete_token();
    state.conn().execute(
        "UPDATE tasks SET deleted_at = ?2 WHERE (id = ?1 OR parent_id = ?1) AND deleted_at IS NULL",
        params![id, token],
    )?;
    Ok(token)
}

#[tauri::command]
pub fn tasks_by_project(state: State<AppState>, project_id: i64) -> Res<Vec<Task>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(&format!(
        "{TASK_SELECT} AND t.project_id = ?1 ORDER BY t.done, t.position, t.id"
    ))?;
    let rows = stmt.query_map([project_id], task_from)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn task_get(state: State<AppState>, id: i64) -> Res<Task> {
    get_task(&state.conn(), id)
}

#[tauri::command]
pub fn subtasks(state: State<AppState>, parent_id: i64) -> Res<Vec<Task>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(&format!("{TASK_SELECT} AND t.parent_id = ?1 ORDER BY t.position, t.id"))?;
    let rows = stmt.query_map([parent_id], task_from)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------- notas

#[tauri::command]
pub fn note_create(state: State<AppState>, project_id: i64, section_id: Option<i64>, note_date: Option<String>) -> Res<Note> {
    let conn = state.conn();
    ensure_project(&conn, project_id)?;
    let section = section_in_project(&conn, section_id, project_id)?;
    let ts = now();
    conn.execute(
        "INSERT INTO notes (project_id, section_id, note_date, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        params![project_id, section, valid_date(note_date)?, ts],
    )?;
    get_note(&conn, conn.last_insert_rowid())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotePatch {
    id: i64,
    title: String,
    body: String,
    note_date: Option<String>,
    remind_at: Option<String>,
    project_id: i64,
    section_id: Option<i64>,
}

#[tauri::command]
pub fn note_update(state: State<AppState>, input: NotePatch) -> Res<Note> {
    let conn = state.conn();
    get_note(&conn, input.id)?;
    ensure_project(&conn, input.project_id)?;
    let section = section_in_project(&conn, input.section_id, input.project_id)?;
    let title = clean_text(input.title.trim(), 500).replace(['\n', '\t'], " ");
    conn.execute(
        "UPDATE notes SET title = ?2, body = ?3, note_date = ?4, remind_at = ?5,
                reminded = CASE WHEN remind_at IS ?5 THEN reminded ELSE 0 END,
                project_id = ?6, section_id = ?7, updated_at = ?8
         WHERE id = ?1",
        params![
            input.id,
            title,
            clean_text(&input.body, 2_000_000),
            valid_date(input.note_date)?,
            valid_datetime(input.remind_at)?,
            input.project_id,
            section,
            now()
        ],
    )?;
    get_note(&conn, input.id)
}

#[tauri::command]
pub fn note_delete(state: State<AppState>, id: i64) -> Res<String> {
    let token = delete_token();
    state.conn().execute(
        "UPDATE notes SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, token],
    )?;
    Ok(token)
}

#[tauri::command]
pub fn notes_by_project(state: State<AppState>, project_id: i64) -> Res<Vec<Note>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(&format!("{NOTE_SELECT} AND n.project_id = ?1 ORDER BY n.updated_at DESC"))?;
    let rows = stmt.query_map([project_id], note_from)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn note_get(state: State<AppState>, id: i64) -> Res<Note> {
    get_note(&state.conn(), id)
}

// ---------------------------------------------------------------- agenda / filtros

#[derive(Serialize)]
pub struct Agenda {
    tasks: Vec<Task>,
    notes: Vec<Note>,
    overdue: Vec<Task>,
    events: Vec<Event>,
}

/// Tarefas e notas com data entre `from` e `to` (inclusive).
/// Se `overdue_before` vier preenchido, inclui as tarefas abertas atrasadas.
#[tauri::command]
pub fn agenda(state: State<AppState>, from: String, to: String, overdue_before: Option<String>) -> Res<Agenda> {
    let from = valid_date(Some(from))?.unwrap_or_default();
    let to = valid_date(Some(to))?.unwrap_or_default();
    let conn = state.conn();

    let tasks = conn
        .prepare(&format!(
            "{TASK_SELECT} AND t.due_date BETWEEN ?1 AND ?2 ORDER BY t.due_date, t.done, t.remind_at IS NULL, t.remind_at, t.position"
        ))?
        .query_map(params![from, to], task_from)?
        .collect::<rusqlite::Result<_>>()?;

    let notes = conn
        .prepare(&format!("{NOTE_SELECT} AND n.note_date BETWEEN ?1 AND ?2 ORDER BY n.note_date, n.updated_at DESC"))?
        .query_map(params![from, to], note_from)?
        .collect::<rusqlite::Result<_>>()?;

    let overdue = match valid_date(overdue_before)? {
        None => Vec::new(),
        Some(before) => conn
            .prepare(&format!(
                "{TASK_SELECT} AND t.done = 0 AND t.due_date < ?1 ORDER BY t.due_date, t.position"
            ))?
            .query_map([before], task_from)?
            .collect::<rusqlite::Result<_>>()?,
    };

    // Eventos que tocam o intervalo (inclusive períodos que começaram antes).
    let events = conn
        .prepare(&format!(
            "{EVENT_SELECT} AND e.start_date <= ?2 AND COALESCE(e.end_date, e.start_date) >= ?1
             ORDER BY e.start_date, e.start_time IS NOT NULL, e.start_time, e.id"
        ))?
        .query_map(params![from, to], event_from)?
        .collect::<rusqlite::Result<_>>()?;

    Ok(Agenda { tasks, notes, overdue, events })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    today: i64,
    overdue: i64,
    tomorrow: i64,
    week: i64,
    next_week: i64,
    events: i64,
    projects: Vec<(i64, i64)>,
}

#[tauri::command]
pub fn counts(
    state: State<AppState>,
    today: String,
    tomorrow: String,
    week_start: String,
    week_end: String,
    next_start: String,
    next_end: String,
) -> Res<Counts> {
    let conn = state.conn();
    let count = |sql: &str, p: &[&dyn rusqlite::ToSql]| -> Res<i64> {
        Ok(conn.query_row(sql, p, |r| r.get(0))?)
    };
    let base = "SELECT COUNT(*) FROM tasks WHERE deleted_at IS NULL AND done = 0";
    let projects = conn
        .prepare(
            "SELECT project_id, COUNT(*) FROM tasks
             WHERE deleted_at IS NULL AND done = 0 AND parent_id IS NULL GROUP BY project_id",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Counts {
        today: count(&format!("{base} AND due_date = ?1"), &[&today])?,
        overdue: count(&format!("{base} AND due_date < ?1"), &[&today])?,
        tomorrow: count(&format!("{base} AND due_date = ?1"), &[&tomorrow])?,
        week: count(&format!("{base} AND due_date BETWEEN ?1 AND ?2"), &[&week_start, &week_end])?,
        next_week: count(&format!("{base} AND due_date BETWEEN ?1 AND ?2"), &[&next_start, &next_end])?,
        events: count(
            "SELECT COUNT(*) FROM events WHERE deleted_at IS NULL AND COALESCE(end_date, start_date) >= ?1",
            &[&today],
        )?,
        projects,
    })
}

// ---------------------------------------------------------------- links úteis

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    id: i64,
    project_id: i64,
    section_id: Option<i64>,
    title: String,
    description: String,
    url: String,
    position: i64,
    created_at: String,
}

const LINK_SELECT: &str = "
    SELECT id, project_id, section_id, title, description, url, position, created_at
    FROM links WHERE deleted_at IS NULL";

fn link_from(r: &Row) -> rusqlite::Result<Link> {
    Ok(Link {
        id: r.get(0)?,
        project_id: r.get(1)?,
        section_id: r.get(2)?,
        title: r.get(3)?,
        description: r.get(4)?,
        url: r.get(5)?,
        position: r.get(6)?,
        created_at: r.get(7)?,
    })
}

fn get_link(conn: &Connection, id: i64) -> Res<Link> {
    conn.query_row(&format!("{LINK_SELECT} AND id = ?1"), [id], link_from)
        .optional()?
        .ok_or_else(|| AppError("Link não encontrado".into()))
}

/// Aceita "site.com", "www.site.com/x", "https://..." e devolve uma URL http(s)
/// válida. Recusa qualquer outro esquema (javascript:, file:, etc.).
pub fn normalize_url(input: &str) -> Res<String> {
    let s = input.trim();
    if s.is_empty() {
        return fail("Informe o endereço do link");
    }
    if s.chars().any(|c| c.is_whitespace() || c.is_control()) || s.len() > 4000 {
        return fail("Endereço inválido: não pode ter espaços");
    }
    let lower = s.to_ascii_lowercase();
    let url = if lower.starts_with("http://") || lower.starts_with("https://") {
        s.to_string()
    } else if s.contains("://") || lower.starts_with("javascript:") || lower.starts_with("data:") {
        return fail("Use um endereço que comece com http:// ou https://");
    } else {
        format!("https://{s}")
    };
    let rest = &url[url.find("://").map(|i| i + 3).unwrap_or(0)..];
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = host.rsplit('@').next().unwrap_or("");
    let name = host.split(':').next().unwrap_or("");
    let ok = !name.is_empty()
        && (name.contains('.') || name == "localhost")
        && name.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        && !name.starts_with('.')
        && !name.ends_with('.');
    if !ok {
        return fail("Endereço inválido. Exemplo: https://site.com.br");
    }
    Ok(url)
}

/// Nome padrão a partir do domínio: "https://www.github.com/x" → "github.com".
fn title_from_url(url: &str) -> String {
    let rest = &url[url.find("://").map(|i| i + 3).unwrap_or(0)..];
    let host = rest.split(['/', '?', '#', ':']).next().unwrap_or(rest);
    host.trim_start_matches("www.").to_string()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkInput {
    id: Option<i64>,
    project_id: i64,
    section_id: Option<i64>,
    title: String,
    description: String,
    url: String,
}

#[tauri::command]
pub fn links_by_project(state: State<AppState>, project_id: i64) -> Res<Vec<Link>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(&format!("{LINK_SELECT} AND project_id = ?1 ORDER BY position, id"))?;
    let rows = stmt.query_map([project_id], link_from)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Cria (sem `id`) ou atualiza (com `id`) um link.
#[tauri::command]
pub fn link_save(state: State<AppState>, input: LinkInput) -> Res<Link> {
    let conn = state.conn();
    ensure_project(&conn, input.project_id)?;
    let section = section_in_project(&conn, input.section_id, input.project_id)?;
    let url = normalize_url(&input.url)?;
    let title = clean_title(&input.title, &title_from_url(&url));
    let description = clean_text(input.description.trim(), 2000);
    let ts = now();

    let id = match input.id {
        Some(id) => {
            get_link(&conn, id)?;
            conn.execute(
                "UPDATE links SET project_id = ?2, section_id = ?3, title = ?4, description = ?5,
                        url = ?6, updated_at = ?7 WHERE id = ?1",
                params![id, input.project_id, section, title, description, url, ts],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO links (project_id, section_id, title, description, url, position, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5,
                         (SELECT COALESCE(MAX(position), 0) + 1 FROM links WHERE project_id = ?1), ?6, ?6)",
                params![input.project_id, section, title, description, url, ts],
            )?;
            conn.last_insert_rowid()
        }
    };
    get_link(&conn, id)
}

#[tauri::command]
pub fn link_delete(state: State<AppState>, id: i64) -> Res<String> {
    let token = delete_token();
    state.conn().execute(
        "UPDATE links SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, token],
    )?;
    Ok(token)
}

/// Abre o link no navegador padrão. Revalida a URL: só http(s) sai do app.
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> Res<()> {
    let url = normalize_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError(format!("Não foi possível abrir o navegador: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_urls() {
        assert_eq!(normalize_url("github.com").unwrap(), "https://github.com");
        assert_eq!(normalize_url("  https://a.com/x?y=1 ").unwrap(), "https://a.com/x?y=1");
        assert_eq!(normalize_url("http://localhost:3000").unwrap(), "http://localhost:3000");
        assert_eq!(normalize_url("www.unicamp.br/dac").unwrap(), "https://www.unicamp.br/dac");
        assert!(normalize_url("javascript:alert(1)").is_err());
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("ftp://a.com").is_err());
        assert!(normalize_url("não é url").is_err());
        assert!(normalize_url("abc").is_err());
        assert!(normalize_url("").is_err());
    }

    #[test]
    fn titulo_padrao_vem_do_dominio() {
        assert_eq!(title_from_url("https://www.github.com/medeirosdev"), "github.com");
        assert_eq!(title_from_url("http://localhost:3000/x"), "localhost");
    }
}

// ---------------------------------------------------------------- eventos

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    id: i64,
    project_id: i64,
    title: String,
    description: String,
    start_date: String,
    end_date: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
    remind_before: Option<i64>,
    remind_at: Option<String>,
    created_at: String,
    project_name: String,
    project_color: String,
}

const EVENT_SELECT: &str = "
    SELECT e.id, e.project_id, e.title, e.description, e.start_date, e.end_date,
           e.start_time, e.end_time, e.remind_before, e.remind_at, e.created_at, p.name, p.color
    FROM events e
    JOIN projects p ON p.id = e.project_id
    WHERE e.deleted_at IS NULL";

fn event_from(r: &Row) -> rusqlite::Result<Event> {
    Ok(Event {
        id: r.get(0)?,
        project_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        start_date: r.get(4)?,
        end_date: r.get(5)?,
        start_time: r.get(6)?,
        end_time: r.get(7)?,
        remind_before: r.get(8)?,
        remind_at: r.get(9)?,
        created_at: r.get(10)?,
        project_name: r.get(11)?,
        project_color: r.get(12)?,
    })
}

fn get_event(conn: &Connection, id: i64) -> Res<Event> {
    conn.query_row(&format!("{EVENT_SELECT} AND e.id = ?1"), [id], event_from)
        .optional()?
        .ok_or_else(|| AppError("Evento não encontrado".into()))
}

fn valid_time(t: Option<String>) -> Res<Option<String>> {
    match t.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => chrono::NaiveTime::parse_from_str(&s, "%H:%M")
            .map(|t| Some(t.format("%H:%M").to_string()))
            .or_else(|_| fail("Horário inválido (use HH:MM)")),
    }
}

/// Horário de referência para avisos de eventos de dia inteiro.
const ALL_DAY_REMIND_TIME: &str = "09:00";
/// Maior antecedência aceita para o aviso: 30 dias.
const MAX_REMIND_BEFORE: i64 = 30 * 24 * 60;

/// Calcula quando avisar: início do evento menos a antecedência.
/// Devolve (remind_at, já_avisado). Se o momento do aviso já passou mas o
/// evento ainda vai acontecer, avisa assim que possível; se o evento já
/// começou, não avisa (seria só ruído).
fn compute_remind(
    start_date: &str,
    start_time: Option<&str>,
    before_min: Option<i64>,
    now: NaiveDateTime,
) -> (Option<String>, bool) {
    let Some(before) = before_min else { return (None, false) };
    let start = format!("{start_date}T{}", start_time.unwrap_or(ALL_DAY_REMIND_TIME));
    let Ok(start) = NaiveDateTime::parse_from_str(&start, "%Y-%m-%dT%H:%M") else { return (None, false) };
    let at = start - chrono::Duration::minutes(before);
    let already = at <= now && start <= now;
    (Some(at.format("%Y-%m-%dT%H:%M").to_string()), already)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    id: Option<i64>,
    project_id: i64,
    title: String,
    description: String,
    start_date: String,
    end_date: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
    remind_before: Option<i64>,
}

#[tauri::command]
pub fn events_by_project(state: State<AppState>, project_id: i64) -> Res<Vec<Event>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(&format!(
        "{EVENT_SELECT} AND e.project_id = ?1 ORDER BY e.start_date, e.start_time IS NOT NULL, e.start_time, e.id"
    ))?;
    let rows = stmt.query_map([project_id], event_from)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Eventos de todos os projetos (tela "Todos os eventos").
#[tauri::command]
pub fn events_all(state: State<AppState>) -> Res<Vec<Event>> {
    let conn = state.conn();
    let mut stmt = conn.prepare(&format!(
        "{EVENT_SELECT} AND p.deleted_at IS NULL ORDER BY e.start_date, e.start_time IS NOT NULL, e.start_time, e.id"
    ))?;
    let rows = stmt.query_map([], event_from)?.collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// Cria (sem `id`) ou atualiza (com `id`) um evento.
#[tauri::command]
pub fn event_save(state: State<AppState>, input: EventInput) -> Res<Event> {
    let conn = state.conn();
    ensure_project(&conn, input.project_id)?;

    let Some(start_date) = valid_date(Some(input.start_date))? else {
        return fail("Informe a data do evento");
    };
    // Fim igual ao início (ou antes dele) vira evento de um dia só.
    let end_date = valid_date(input.end_date)?.filter(|e| e > &start_date);
    let start_time = valid_time(input.start_time)?;
    let mut end_time = if start_time.is_some() { valid_time(input.end_time)? } else { None };
    if end_date.is_none() {
        if let (Some(s), Some(e)) = (&start_time, &end_time) {
            if e <= s {
                end_time = None;
            }
        }
    }
    let before = match input.remind_before {
        Some(m) if (0..=MAX_REMIND_BEFORE).contains(&m) => Some(m),
        Some(_) => return fail("Antecedência do aviso inválida"),
        None => None,
    };

    let title = clean_title(&input.title, "Evento");
    let description = clean_text(input.description.trim(), 5000);
    let (remind_at, reminded) =
        compute_remind(&start_date, start_time.as_deref(), before, Local::now().naive_local());
    let ts = now();

    let id = match input.id {
        Some(id) => {
            get_event(&conn, id)?;
            conn.execute(
                "UPDATE events SET project_id = ?2, title = ?3, description = ?4, start_date = ?5,
                        end_date = ?6, start_time = ?7, end_time = ?8, remind_before = ?9,
                        reminded = CASE WHEN remind_at IS ?10 THEN reminded ELSE ?11 END,
                        remind_at = ?10, updated_at = ?12
                 WHERE id = ?1",
                params![
                    id, input.project_id, title, description, start_date, end_date, start_time,
                    end_time, before, remind_at, reminded, ts
                ],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO events (project_id, title, description, start_date, end_date, start_time,
                                     end_time, remind_before, remind_at, reminded, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    input.project_id, title, description, start_date, end_date, start_time,
                    end_time, before, remind_at, reminded, ts
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    get_event(&conn, id)
}

#[tauri::command]
pub fn event_delete(state: State<AppState>, id: i64) -> Res<String> {
    let token = delete_token();
    state.conn().execute(
        "UPDATE events SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, token],
    )?;
    Ok(token)
}

#[cfg(test)]
mod event_tests {
    use super::*;

    fn at(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").unwrap()
    }

    #[test]
    fn aviso_relativo_ao_inicio() {
        let now = at("2026-10-10T08:00");
        assert_eq!(
            compute_remind("2026-10-20", Some("14:00"), Some(60), now),
            (Some("2026-10-20T13:00".into()), false)
        );
        // Dia inteiro: referência às 09:00; 1 dia antes.
        assert_eq!(
            compute_remind("2026-10-20", None, Some(1440), now),
            (Some("2026-10-19T09:00".into()), false)
        );
        assert_eq!(compute_remind("2026-10-20", None, None, now), (None, false));
    }

    #[test]
    fn aviso_atrasado_so_dispara_se_o_evento_ainda_vai_acontecer() {
        let now = at("2026-10-10T08:00");
        // Evento hoje às 10h com aviso de 1 semana antes: avisa já.
        assert_eq!(
            compute_remind("2026-10-10", Some("10:00"), Some(10080), now),
            (Some("2026-10-03T10:00".into()), false)
        );
        // Evento que já começou: não avisa.
        assert_eq!(
            compute_remind("2026-10-09", Some("10:00"), Some(0), now),
            (Some("2026-10-09T10:00".into()), true)
        );
    }
}

// ---------------------------------------------------------------- backup no GitHub

#[tauri::command]
pub fn sync_status(state: State<AppState>) -> crate::sync::Status {
    crate::sync::status(&state.conn())
}

#[tauri::command]
pub fn sync_configure(state: State<AppState>, repo: String, enabled: bool) -> Res<crate::sync::Status> {
    let repo = repo.trim().to_string();
    if enabled || !repo.is_empty() {
        crate::sync::validate_repo(&repo)?;
    }
    let conn = state.conn();
    for (key, value) in [
        (crate::sync::KEY_REPO, repo.as_str()),
        (crate::sync::KEY_ENABLED, if enabled { "true" } else { "false" }),
    ] {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
    }
    Ok(crate::sync::status(&conn))
}

/// Envia agora. Roda fora da thread principal: o git pode demorar.
#[tauri::command]
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Res<String> {
    let conn = state.shared();
    let dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || crate::sync::run(&conn, &dir, Some(&app)))
        .await
        .map_err(|e| AppError(e.to_string()))?
        .map_err(AppError)
}
