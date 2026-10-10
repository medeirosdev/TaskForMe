//! Comandos chamados pela interface. Toda entrada é validada aqui: a UI nunca
//! consegue gravar data inválida, referência quebrada ou texto vazio.

use std::sync::atomic::Ordering;

use chrono::{Local, NaiveDate, NaiveDateTime};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

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
    for table in ["projects", "sections", "tasks", "notes"] {
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

    Ok(Agenda { tasks, notes, overdue })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    today: i64,
    overdue: i64,
    tomorrow: i64,
    week: i64,
    projects: Vec<(i64, i64)>,
}

#[tauri::command]
pub fn counts(state: State<AppState>, today: String, tomorrow: String, week_start: String, week_end: String) -> Res<Counts> {
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
        projects,
    })
}
