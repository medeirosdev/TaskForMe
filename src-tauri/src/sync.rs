//! Backup automático em um repositório Git (ex.: repositório privado no GitHub).
//!
//! Fluxo: exporta o banco como SQL (texto, ordenado → diffs pequenos), grava numa
//! cópia local do repositório e faz commit + push. Roda ao abrir o app e uma vez
//! por dia. Nunca trava o app: tudo em thread própria, com tempo limite, e a
//! falha só é registrada (com nova tentativa depois).

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use chrono::{Local, NaiveDateTime};
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::logger::log;
use crate::Shared;

const REPO_DIR: &str = "sync-repo";
const DUMP_FILE: &str = "taskforme.sql";
const GIT_TIMEOUT: Duration = Duration::from_secs(90);
/// Com o app aberto, envia de novo quando o último envio tiver mais de ~20 h.
const RESEND_AFTER_HOURS: i64 = 20;
/// Esperas entre tentativas quando falha (rede ainda subindo no boot, etc.).
const RETRY_MINUTES: [u64; 4] = [2, 10, 30, 60];

static RUNNING: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------- configurações

pub const KEY_REPO: &str = "syncRepo";
pub const KEY_ENABLED: &str = "syncEnabled";
const KEY_LAST: &str = "syncLast";
const KEY_ERROR: &str = "syncError";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub repo: String,
    pub last: Option<String>,
    pub error: Option<String>,
    pub running: bool,
}

fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

fn set_setting(conn: &Connection, key: &str, value: Option<&str>) {
    let r = match value {
        Some(v) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, v],
        ),
        None => conn.execute("DELETE FROM settings WHERE key = ?1", [key]),
    };
    if let Err(e) = r {
        log(&format!("Sync: falha ao gravar configuração {key}: {e}"));
    }
}

pub fn status(conn: &Connection) -> Status {
    Status {
        enabled: get_setting(conn, KEY_ENABLED).as_deref() == Some("true"),
        repo: get_setting(conn, KEY_REPO).unwrap_or_default(),
        last: get_setting(conn, KEY_LAST),
        error: get_setting(conn, KEY_ERROR),
        running: RUNNING.load(Ordering::SeqCst),
    }
}

/// Aceita endereços SSH (git@host:usuario/repo.git, ssh://...) e HTTPS.
/// Recusa qualquer coisa que o git pudesse interpretar como opção ou comando.
pub fn validate_repo(url: &str) -> Result<String, String> {
    let u = url.trim();
    if u.is_empty() {
        return Err("Informe o endereço do repositório".into());
    }
    let bad = u.starts_with('-')
        || u.len() > 500
        || u.chars().any(|c| c.is_whitespace() || c.is_control() || "'\"`$;|&<>\\".contains(c));
    let scp_like = {
        // git@github.com:usuario/repo.git
        let (user_host, path) = u.split_once(':').unwrap_or(("", ""));
        user_host.contains('@') && !user_host.contains('/') && !path.is_empty() && !path.starts_with("//")
    };
    let ok_scheme = u.starts_with("https://") || u.starts_with("ssh://") || scp_like;
    if bad || !ok_scheme {
        return Err(
            "Endereço inválido. Use o formato git@github.com:usuario/repositorio.git ou https://github.com/usuario/repositorio.git"
                .into(),
        );
    }
    Ok(u.to_string())
}

// ---------------------------------------------------------------- exportação

fn sql_value(v: ValueRef) -> String {
    match v {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => format!("{f:?}"),
        ValueRef::Text(t) => format!("'{}'", String::from_utf8_lossy(t).replace('\'', "''")),
        ValueRef::Blob(b) => format!("X'{}'", b.iter().map(|x| format!("{x:02X}")).collect::<String>()),
    }
}

/// Exporta o banco inteiro como SQL. Sem data/hora no conteúdo: se nada
/// mudou, o arquivo fica idêntico e não gera commit.
pub fn dump(conn: &Connection) -> rusqlite::Result<String> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let mut out = String::from(
        "-- Backup do TaskForMe. Para restaurar, veja o README.md deste repositório.\n\
         PRAGMA foreign_keys=OFF;\nBEGIN TRANSACTION;\n",
    );

    let tables: Vec<(String, String)> = conn
        .prepare(
            "SELECT name, sql FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY rowid",
        )?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    for (name, sql) in &tables {
        out.push_str(sql);
        out.push_str(";\n");
        // Configurações do próprio backup não vão para o backup.
        let filter = if name == "settings" { " WHERE key NOT LIKE 'sync%'" } else { "" };
        let order = if name == "settings" { "key" } else { "rowid" };
        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{name}\"{filter} ORDER BY {order}"))?;
        let cols = stmt.column_count();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let values: Vec<String> = (0..cols).map(|i| row.get_ref(i).map(sql_value)).collect::<Result<_, _>>()?;
            out.push_str(&format!("INSERT INTO \"{name}\" VALUES({});\n", values.join(",")));
        }
    }

    let indexes: Vec<String> = conn
        .prepare("SELECT sql FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY rowid")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for sql in indexes {
        out.push_str(&sql);
        out.push_str(";\n");
    }

    out.push_str(&format!("PRAGMA user_version={version};\nCOMMIT;\n"));
    Ok(out)
}

const REPO_README: &str = "# Backup do TaskForMe

Este repositório é atualizado automaticamente pelo app TaskForMe
(ao abrir o computador e uma vez por dia). **Mantenha-o privado.**

`taskforme.sql` contém todos os dados: projetos, tarefas, notas, eventos e links.

## Restaurar

1. Feche o TaskForMe (inclusive na bandeja → Sair).
2. Renomeie o arquivo atual `taskforme.db` (para guardar uma cópia) na pasta de dados:
   - Linux: `~/.local/share/com.medeiros.taskforme/`
   - Windows: `%APPDATA%\\com.medeiros.taskforme\\`
3. Recrie o banco a partir deste backup:

```bash
sqlite3 taskforme.db < taskforme.sql
```

4. Abra o TaskForMe.
";

// ---------------------------------------------------------------- git

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Nunca pedir senha/usuário: sem terminal, o processo ficaria preso.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never");
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new");
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            "Git não encontrado. Instale o Git para usar o backup no GitHub.".to_string()
        } else {
            format!("Não foi possível executar o git: {e}")
        }
    })?;

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > GIT_TIMEOUT => {
                let _ = child.kill();
                return Err(format!("git {} demorou demais (sem internet?)", args.first().unwrap_or(&"")));
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut stdout);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut stderr);
    }
    if status.success() {
        Ok(stdout)
    } else {
        Err(friendly_git_error(stderr.trim()))
    }
}

fn friendly_git_error(stderr: &str) -> String {
    let s = stderr.to_lowercase();
    if s.contains("permission denied") || s.contains("authentication failed") || s.contains("could not read username") {
        "Sem permissão no repositório. Confira o endereço e se sua chave SSH (ou login do Git) tem acesso. \
         Dica: prefira o endereço SSH (git@github.com:usuario/repositorio.git)."
            .into()
    } else if s.contains("repository not found") || s.contains("does not appear to be a git repository") {
        "Repositório não encontrado. Confira o endereço e se ele já foi criado no GitHub.".into()
    } else if s.contains("could not resolve host") || s.contains("network is unreachable") || s.contains("connection timed out") {
        "Sem conexão com a internet. Vou tentar de novo mais tarde.".into()
    } else {
        let last = stderr.lines().last().unwrap_or("erro desconhecido");
        format!("Falha no git: {last}")
    }
}

/// https://github.com/usuario/repo(.git) → git@github.com:usuario/repo.git
fn https_to_ssh(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let (host, path) = rest.split_once('/')?;
    if host.contains('@') || host.contains(':') || path.is_empty() {
        return None;
    }
    let path = path.trim_end_matches('/');
    let path = if path.ends_with(".git") { path.to_string() } else { format!("{path}.git") };
    Some(format!("git@{host}:{path}"))
}

fn is_auth_error(e: &str) -> bool {
    e.starts_with("Sem permissão")
}

/// Garante que `dir` é um repositório apontando para `url` e alinhado ao remoto.
fn prepare_repo(dir: &Path, url: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    if !dir.join(".git").exists() {
        git(dir, &["init", "-q"])?;
        git(dir, &["remote", "add", "origin", url])?;
    } else {
        git(dir, &["remote", "set-url", "origin", url])?;
    }
    // Repositório recém-criado no GitHub não tem branch ainda: tudo bem.
    match git(dir, &["fetch", "-q", "origin", "main"]) {
        Ok(_) => {
            // Mantém os arquivos locais e passa a seguir o histórico do remoto:
            // o próximo commit entra por cima dele (sem conflitos).
            git(dir, &["reset", "-q", "--soft", "FETCH_HEAD"])?;
        }
        Err(e) if e.contains("Sem permissão") || e.contains("não encontrado") || e.contains("conexão") => {
            return Err(e)
        }
        Err(_) => {} // branch main ainda não existe no remoto
    }
    Ok(())
}

fn has_identity(dir: &Path) -> bool {
    git(dir, &["config", "user.email"]).map(|s| !s.trim().is_empty()).unwrap_or(false)
}

fn commit_and_push(dir: &Path) -> Result<bool, String> {
    git(dir, &["add", "-A"])?;
    // Nada mudou desde o último envio?
    let changed = git(dir, &["diff", "--cached", "--quiet"]).is_err();
    let has_head = git(dir, &["rev-parse", "--verify", "-q", "HEAD"]).is_ok();
    if changed || !has_head {
        let msg = format!("Backup {}", Local::now().format("%Y-%m-%d %H:%M"));
        // Usa a identidade do Git do usuário; só define uma se não houver nenhuma.
        let mut args: Vec<&str> = Vec::new();
        if !has_identity(dir) {
            args.extend(["-c", "user.name=TaskForMe", "-c", "user.email=taskforme@localhost"]);
        }
        args.extend(["commit", "-q", "--allow-empty", "-m", &msg]);
        git(dir, &args)?;
    }
    git(dir, &["push", "-q", "origin", "HEAD:refs/heads/main"])?;
    Ok(changed)
}

/// Executa um envio completo. Devolve uma mensagem curta de resultado.
pub fn run(conn: &Shared, data_dir: &Path, app: Option<&AppHandle>) -> Result<String, String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Err("Um envio já está em andamento".into());
    }
    let notify = |app: Option<&AppHandle>, conn: &Shared| {
        if let Some(app) = app {
            let st = status(&conn.lock().unwrap_or_else(|e| e.into_inner()));
            let _ = app.emit("sync-status", st);
        }
    };
    notify(app, conn);

    let result = (|| -> Result<String, String> {
        // Exporta segurando o banco só pelo tempo da leitura.
        let (url, sql) = {
            let c = conn.lock().unwrap_or_else(|e| e.into_inner());
            let url = get_setting(&c, KEY_REPO).ok_or_else(|| "Configure o endereço do repositório".to_string())?;
            (validate_repo(&url)?, dump(&c).map_err(|e| format!("Falha ao exportar os dados: {e}"))?)
        };
        let dir: PathBuf = data_dir.join(REPO_DIR);
        match prepare_repo(&dir, &url) {
            Ok(()) => {}
            // HTTPS sem login salvo do Git: tenta o mesmo repositório via SSH
            // (a chave SSH costuma já estar configurada). Se der certo, adota.
            Err(e) if is_auth_error(&e) => {
                let ssh = https_to_ssh(&url).ok_or(e.clone())?;
                prepare_repo(&dir, &ssh).map_err(|_| e)?;
                let c = conn.lock().unwrap_or_else(|e| e.into_inner());
                set_setting(&c, KEY_REPO, Some(&ssh));
                log(&format!("Sync: HTTPS sem login; passando a usar {ssh}"));
            }
            Err(e) => return Err(e),
        }
        fs::write(dir.join(DUMP_FILE), sql).map_err(|e| e.to_string())?;
        fs::write(dir.join("README.md"), REPO_README).map_err(|e| e.to_string())?;
        let changed = commit_and_push(&dir)?;
        Ok(if changed { "Dados enviados".to_string() } else { "Nada mudou desde o último envio".to_string() })
    })();

    {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        match &result {
            Ok(_) => {
                set_setting(&c, KEY_LAST, Some(&Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()));
                set_setting(&c, KEY_ERROR, None);
            }
            Err(e) => set_setting(&c, KEY_ERROR, Some(e)),
        }
    }
    RUNNING.store(false, Ordering::SeqCst);
    match &result {
        Ok(m) => log(&format!("Sync: {m}")),
        Err(e) => log(&format!("Sync falhou: {e}")),
    }
    notify(app, conn);
    result
}

/// Agenda: envia logo após abrir (dando tempo da rede subir no boot), repete
/// a cada ~20 h com o app aberto e tenta de novo com espera crescente se falhar.
pub fn spawn_scheduler(app: AppHandle, conn: Shared, data_dir: PathBuf) {
    let _ = thread::Builder::new().name("sync".into()).spawn(move || {
        thread::sleep(Duration::from_secs(20));
        let mut sent_this_session = false;
        let mut failures = 0usize;
        loop {
            let (enabled, last) = {
                let c = conn.lock().unwrap_or_else(|e| e.into_inner());
                let st = status(&c);
                (st.enabled && !st.repo.is_empty(), st.last)
            };
            let stale = last
                .and_then(|l| NaiveDateTime::parse_from_str(&l, "%Y-%m-%dT%H:%M:%S").ok())
                .map(|l| (Local::now().naive_local() - l).num_hours() >= RESEND_AFTER_HOURS)
                .unwrap_or(true);

            let mut wait = Duration::from_secs(30 * 60);
            if enabled && (!sent_this_session || stale) {
                let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(&conn, &data_dir, Some(&app)).is_ok()
                }))
                .unwrap_or(false);
                if ok {
                    sent_this_session = true;
                    failures = 0;
                } else {
                    wait = Duration::from_secs(60 * RETRY_MINUTES[failures.min(RETRY_MINUTES.len() - 1)]);
                    failures += 1;
                }
            }
            thread::sleep(wait);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valida_enderecos_de_repositorio() {
        assert!(validate_repo("git@github.com:medeirosdev/taskforme-dados.git").is_ok());
        assert!(validate_repo("https://github.com/medeirosdev/taskforme-dados.git").is_ok());
        assert!(validate_repo("ssh://git@github.com/medeirosdev/x.git").is_ok());
        assert!(validate_repo("--upload-pack=touch /tmp/x").is_err());
        assert!(validate_repo("git@github.com:a/b.git; rm -rf ~").is_err());
        assert!(validate_repo("http://github.com/a/b.git").is_err());
        assert!(validate_repo("file:///etc").is_err());
        assert!(validate_repo("").is_err());
    }

    #[test]
    fn envia_para_repositorio_e_sobrevive_a_historico_remoto() {
        let base = std::env::temp_dir().join(format!(
            "taskforme-git-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let remote = base.join("remote.git");
        let local = base.join("local");
        fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "-q", "--bare"]).unwrap();
        let url = remote.to_string_lossy().to_string();

        // Primeiro envio para repositório vazio.
        prepare_repo(&local, &url).unwrap();
        fs::write(local.join(DUMP_FILE), "v1").unwrap();
        assert!(commit_and_push(&local).unwrap());

        // Sem mudanças: não cria commit novo.
        prepare_repo(&local, &url).unwrap();
        assert!(!commit_and_push(&local).unwrap());

        // Outro computador enviou algo antes: o próximo envio entra por cima, sem conflito.
        let other = base.join("other");
        prepare_repo(&other, &url).unwrap();
        fs::write(other.join(DUMP_FILE), "v-outro").unwrap();
        commit_and_push(&other).unwrap();

        prepare_repo(&local, &url).unwrap();
        fs::write(local.join(DUMP_FILE), "v2").unwrap();
        assert!(commit_and_push(&local).unwrap());

        let count = git(&remote, &["rev-list", "--count", "main"]).unwrap();
        assert_eq!(count.trim(), "3");
        let content = git(&remote, &["show", "main:taskforme.sql"]).unwrap();
        assert_eq!(content, "v2");
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn converte_https_para_ssh() {
        assert_eq!(
            https_to_ssh("https://github.com/medeirosdev/taskformedatabase.git").as_deref(),
            Some("git@github.com:medeirosdev/taskformedatabase.git")
        );
        assert_eq!(
            https_to_ssh("https://github.com/medeirosdev/taskformedatabase").as_deref(),
            Some("git@github.com:medeirosdev/taskformedatabase.git")
        );
        assert_eq!(https_to_ssh("git@github.com:a/b.git"), None);
        assert_eq!(https_to_ssh("https://user@github.com/a/b.git"), None);
    }

    #[test]
    fn exportacao_restaura_identica() {
        let dir = std::env::temp_dir().join(format!(
            "taskforme-dump-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let opened = crate::db::open(&dir).unwrap();
        let c = &opened.conn;
        c.execute("INSERT INTO projects (name, created_at) VALUES ('Aspas '' e acentuação ção', 'x')", []).unwrap();
        c.execute(
            "INSERT INTO notes (project_id, title, body, created_at, updated_at)
             VALUES (1, 'Nota', 'linha 1\nlinha 2; DROP TABLE x;', 'x', 'x')",
            [],
        )
        .unwrap();
        c.execute("INSERT INTO settings (key, value) VALUES ('syncRepo', 'segredo'), ('theme', 'dark')", []).unwrap();

        let sql = dump(c).unwrap();
        assert!(!sql.contains("segredo"), "configurações do sync não vão para o backup");
        assert_eq!(dump(c).unwrap(), sql, "exportação determinística");

        let restored = Connection::open_in_memory().unwrap();
        restored.execute_batch(&sql).unwrap();
        let name: String = restored
            .query_row("SELECT name FROM projects WHERE id = 2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "Aspas ' e acentuação ção");
        let body: String = restored.query_row("SELECT body FROM notes", [], |r| r.get(0)).unwrap();
        assert_eq!(body, "linha 1\nlinha 2; DROP TABLE x;");
        let v: i64 = restored.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        let orig: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, orig);
        fs::remove_dir_all(&dir).ok();
    }
}
