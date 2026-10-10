//! Abertura do banco, migrations, backups automáticos e recuperação de falhas.
//!
//! Estratégia de resiliência:
//! 1. SQLite em modo WAL + synchronous=FULL: nenhuma escrita confirmada se perde
//!    mesmo se o PC desligar no meio.
//! 2. Na abertura roda `quick_check`. Se o arquivo estiver corrompido, ele é
//!    renomeado (nunca apagado) e o backup mais recente é restaurado.
//! 3. Backup diário automático (mantém os últimos 14) e antes de cada migration.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Local;
use rusqlite::{backup::Backup, Connection, OpenFlags};

use crate::logger::log;

const DB_FILE: &str = "taskforme.db";
const BACKUP_DIR: &str = "backups";
const KEEP_BACKUPS: usize = 14;
const BACKUP_INTERVAL_HOURS: i64 = 20;
const PURGE_AFTER_DAYS: i64 = 7;
const NEWER_VERSION: &str = "Os dados foram criados por uma versão mais nova do app";

const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE projects (
        id         INTEGER PRIMARY KEY,
        name       TEXT    NOT NULL,
        color      TEXT    NOT NULL DEFAULT '#5e6ad2',
        is_inbox   INTEGER NOT NULL DEFAULT 0,
        position   INTEGER NOT NULL DEFAULT 0,
        created_at TEXT    NOT NULL,
        deleted_at TEXT
    );

    CREATE TABLE sections (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        name       TEXT    NOT NULL,
        position   INTEGER NOT NULL DEFAULT 0,
        collapsed  INTEGER NOT NULL DEFAULT 0,
        deleted_at TEXT
    );

    CREATE TABLE tasks (
        id          INTEGER PRIMARY KEY,
        project_id  INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        section_id  INTEGER REFERENCES sections(id) ON DELETE SET NULL,
        parent_id   INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
        title       TEXT    NOT NULL,
        description TEXT    NOT NULL DEFAULT '',
        due_date    TEXT,
        remind_at   TEXT,
        reminded    INTEGER NOT NULL DEFAULT 0,
        done        INTEGER NOT NULL DEFAULT 0,
        done_at     TEXT,
        position    INTEGER NOT NULL DEFAULT 0,
        created_at  TEXT    NOT NULL,
        updated_at  TEXT    NOT NULL,
        deleted_at  TEXT
    );

    CREATE TABLE notes (
        id         INTEGER PRIMARY KEY,
        project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
        section_id INTEGER REFERENCES sections(id) ON DELETE SET NULL,
        title      TEXT    NOT NULL DEFAULT '',
        body       TEXT    NOT NULL DEFAULT '',
        note_date  TEXT,
        remind_at  TEXT,
        reminded   INTEGER NOT NULL DEFAULT 0,
        created_at TEXT    NOT NULL,
        updated_at TEXT    NOT NULL,
        deleted_at TEXT
    );

    CREATE INDEX idx_sections_project ON sections(project_id);
    CREATE INDEX idx_tasks_project    ON tasks(project_id);
    CREATE INDEX idx_tasks_parent     ON tasks(parent_id);
    CREATE INDEX idx_tasks_due        ON tasks(due_date);
    CREATE INDEX idx_tasks_remind     ON tasks(remind_at) WHERE reminded = 0;
    CREATE INDEX idx_notes_project    ON notes(project_id);
    CREATE INDEX idx_notes_date       ON notes(note_date);
    CREATE INDEX idx_notes_remind     ON notes(remind_at) WHERE reminded = 0;

    INSERT INTO projects (name, color, is_inbox, position, created_at)
    VALUES ('Entrada', '#8a8f98', 1, 0, strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime'));
    "#,
];

pub struct Opened {
    pub conn: Connection,
    /// Mensagem para mostrar ao usuário quando houve recuperação.
    pub notice: Option<String>,
}

pub fn open(data_dir: &Path) -> Result<Opened, String> {
    fs::create_dir_all(data_dir.join(BACKUP_DIR)).map_err(|e| e.to_string())?;
    let db_path = data_dir.join(DB_FILE);

    match try_open(&db_path, data_dir) {
        Ok(conn) => {
            maintenance(&conn, data_dir);
            Ok(Opened { conn, notice: None })
        }
        // Banco de uma versão mais nova do app não está corrompido: não restaura nada.
        Err(err) if err.starts_with(NEWER_VERSION) => Err(err),
        Err(err) => {
            log(&format!("Banco inválido ({err}). Iniciando recuperação."));
            recover(&db_path, data_dir)
        }
    }
}

fn try_open(db_path: &Path, data_dir: &Path) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
    )
    .map_err(|e| e.to_string())?;

    conn.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(|e| e.to_string())?;

    let check: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    if check != "ok" {
        return Err(format!("quick_check: {check}"));
    }

    migrate(&conn, db_path, data_dir)?;
    Ok(conn)
}

fn migrate(conn: &Connection, db_path: &Path, data_dir: &Path) -> Result<(), String> {
    let version: usize = conn
        .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
        .map_err(|e| e.to_string())? as usize;

    if version > MIGRATIONS.len() {
        // Banco criado por uma versão mais nova do app: não mexe.
        return Err(format!(
            "{NEWER_VERSION} (versão {version}; este app conhece até {}). Atualize o TaskForMe.",
            MIGRATIONS.len()
        ));
    }
    if version == MIGRATIONS.len() {
        return Ok(());
    }

    if version > 0 && db_path.exists() {
        let _ = backup_now(conn, data_dir, "pre-migration");
    }

    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        tx.execute_batch(sql).map_err(|e| format!("migration {}: {e}", i + 1))?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        log(&format!("Migration {} aplicada", i + 1));
    }
    Ok(())
}

fn recover(db_path: &Path, data_dir: &Path) -> Result<Opened, String> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let quarantine = data_dir.join(format!("taskforme.corrupt-{stamp}.db"));
    if db_path.exists() {
        let _ = fs::rename(db_path, &quarantine);
        for ext in ["-wal", "-shm"] {
            let side = PathBuf::from(format!("{}{ext}", db_path.display()));
            if side.exists() {
                let _ = fs::rename(&side, format!("{}{ext}", quarantine.display()));
            }
        }
    }

    for backup in list_backups(data_dir).into_iter().rev() {
        if fs::copy(&backup, db_path).is_err() {
            continue;
        }
        match try_open(db_path, data_dir) {
            Ok(conn) => {
                let name = backup.file_name().unwrap_or_default().to_string_lossy();
                log(&format!("Restaurado a partir de {name}"));
                return Ok(Opened {
                    conn,
                    notice: Some(format!(
                        "O banco de dados estava danificado e foi restaurado do backup {name}. \
                         O arquivo original foi preservado como {}.",
                        quarantine.file_name().unwrap_or_default().to_string_lossy()
                    )),
                });
            }
            Err(e) => {
                log(&format!("Backup {} também inválido: {e}", backup.display()));
                let _ = fs::remove_file(db_path);
            }
        }
    }

    let conn = try_open(db_path, data_dir)?;
    let had_data = quarantine.exists();
    Ok(Opened {
        conn,
        notice: had_data.then(|| {
            "O banco de dados estava danificado e não havia backup válido. \
             Um banco novo foi criado; o arquivo antigo foi preservado na pasta de dados."
                .to_string()
        }),
    })
}

/// Faz o backup automático se o último tiver mais de ~20 h. Chamado na
/// abertura e de hora em hora (o app pode ficar dias aberto na bandeja).
pub fn backup_if_due(conn: &Connection, data_dir: &Path) {
    let needs_backup = list_backups(data_dir)
        .last()
        .and_then(|p| fs::metadata(p).ok()?.modified().ok())
        .map(|t| {
            let age = std::time::SystemTime::now().duration_since(t).unwrap_or_default();
            age.as_secs() as i64 > BACKUP_INTERVAL_HOURS * 3600
        })
        .unwrap_or(true);
    if needs_backup {
        if let Err(e) = backup_now(conn, data_dir, "auto") {
            log(&format!("Falha no backup automático: {e}"));
        }
    }
}

/// Backup diário, limpeza de itens excluídos há mais de 7 dias e otimização.
fn maintenance(conn: &Connection, data_dir: &Path) {
    backup_if_due(conn, data_dir);

    let cutoff = (Local::now() - chrono::Duration::days(PURGE_AFTER_DAYS))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let purge = conn.execute_batch(&format!(
        "BEGIN;
         DELETE FROM tasks    WHERE deleted_at IS NOT NULL AND deleted_at < '{cutoff}';
         DELETE FROM notes    WHERE deleted_at IS NOT NULL AND deleted_at < '{cutoff}';
         DELETE FROM sections WHERE deleted_at IS NOT NULL AND deleted_at < '{cutoff}';
         DELETE FROM projects WHERE deleted_at IS NOT NULL AND deleted_at < '{cutoff}' AND is_inbox = 0;
         COMMIT;"
    ));
    if let Err(e) = purge {
        let _ = conn.execute_batch("ROLLBACK;");
        log(&format!("Falha ao limpar lixeira: {e}"));
    }
    let _ = conn.execute_batch("PRAGMA optimize;");
}

pub fn backup_now(conn: &Connection, data_dir: &Path, tag: &str) -> Result<PathBuf, String> {
    let dir = data_dir.join(BACKUP_DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = format!("taskforme-{}-{tag}.db", Local::now().format("%Y%m%d-%H%M%S"));
    let tmp = dir.join(format!("{name}.part"));
    let dest = dir.join(&name);

    {
        let mut out = Connection::open(&tmp).map_err(|e| e.to_string())?;
        let backup = Backup::new(conn, &mut out).map_err(|e| e.to_string())?;
        backup
            .run_to_completion(256, Duration::from_millis(0), None)
            .map_err(|e| e.to_string())?;
    }
    // Rename atômico: um backup pela metade nunca fica com nome de backup válido.
    fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;

    let backups = list_backups(data_dir);
    if backups.len() > KEEP_BACKUPS {
        for old in &backups[..backups.len() - KEEP_BACKUPS] {
            let _ = fs::remove_file(old);
        }
    }
    Ok(dest)
}

/// Backups ordenados do mais antigo para o mais novo (o nome contém o timestamp).
fn list_backups(data_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(data_dir.join(BACKUP_DIR))
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension().is_some_and(|x| x == "db")
                        && p.file_name()
                            .is_some_and(|n| n.to_string_lossy().starts_with("taskforme-"))
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "taskforme-test-{name}-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn cria_banco_novo_com_entrada_e_backup() {
        let dir = temp_dir("novo");
        let opened = open(&dir).unwrap();
        assert!(opened.notice.is_none());
        let inbox: i64 = opened
            .conn
            .query_row("SELECT COUNT(*) FROM projects WHERE is_inbox = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(inbox, 1);
        assert_eq!(list_backups(&dir).len(), 1);
        // Abrir de novo não duplica a Entrada nem reaplica migrations.
        drop(opened);
        let again = open(&dir).unwrap();
        let inbox: i64 = again.conn.query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0)).unwrap();
        assert_eq!(inbox, 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recupera_banco_corrompido_a_partir_do_backup() {
        let dir = temp_dir("corrompido");
        {
            let opened = open(&dir).unwrap();
            opened
                .conn
                .execute("INSERT INTO projects (name, created_at) VALUES ('Trabalho', 'x')", [])
                .unwrap();
            backup_now(&opened.conn, &dir, "manual").unwrap();
        }
        let db = dir.join(DB_FILE);
        let mut bytes = fs::read(&db).unwrap();
        for b in bytes.iter_mut().skip(100).take(8000) {
            *b = 0x5a;
        }
        fs::write(&db, bytes).unwrap();
        let _ = fs::remove_file(dir.join("taskforme.db-wal"));

        let opened = open(&dir).unwrap();
        assert!(opened.notice.is_some(), "deve avisar o usuário");
        let n: i64 = opened
            .conn
            .query_row("SELECT COUNT(*) FROM projects WHERE name = 'Trabalho'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "dados do backup restaurados");
        let quarantined = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("taskforme.corrupt-"));
        assert!(quarantined, "arquivo corrompido preservado, nunca apagado");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn nao_mexe_em_banco_de_versao_mais_nova() {
        let dir = temp_dir("futuro");
        {
            let opened = open(&dir).unwrap();
            opened.conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let conn = Connection::open(dir.join(DB_FILE)).unwrap();
        assert!(migrate(&conn, &dir.join(DB_FILE), &dir).is_err());
        drop(conn);
        assert!(open(&dir).is_err(), "não deve restaurar backup por cima");
        let quarantined = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with("taskforme.corrupt-"));
        assert!(!quarantined);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn mantem_no_maximo_14_backups() {
        let dir = temp_dir("rotacao");
        let opened = open(&dir).unwrap();
        for i in 0..20 {
            backup_now(&opened.conn, &dir, &format!("t{i:02}")).unwrap();
        }
        assert_eq!(list_backups(&dir).len(), KEEP_BACKUPS);
        fs::remove_dir_all(&dir).ok();
    }
}
