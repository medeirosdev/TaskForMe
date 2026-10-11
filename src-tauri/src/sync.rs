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
const TIMEOUT_PREFIX: &str = "O git demorou demais para responder (sem internet?)";
const BUSY: &str = "Um envio ou restauração já está em andamento";
const NO_REMOTE_BRANCH: &str = "couldn't find remote ref";
/// Com o app aberto, envia de novo quando o último envio tiver mais de ~20 h.
const RESEND_AFTER_HOURS: i64 = 20;
/// Esperas entre tentativas quando falha (rede ainda subindo no boot, etc.).
const RETRY_MINUTES: [u64; 4] = [2, 10, 30, 60];

static RUNNING: AtomicBool = AtomicBool::new(false);

/// Libera a trava de "envio em andamento" ao sair do escopo — inclusive se
/// algo der panic no meio. Sem isso, um erro raro travaria o backup até reiniciar.
struct RunningGuard;

impl RunningGuard {
    fn acquire() -> Option<Self> {
        (!RUNNING.swap(true, Ordering::SeqCst)).then_some(RunningGuard)
    }
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

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

**Pelo app (recomendado):** Configurações → Backup no GitHub → **Restaurar do GitHub**.
O app confere o backup, guarda uma cópia dos dados atuais e só então substitui.

**Manualmente** (precisa do programa `sqlite3`):

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

    // Lê as saídas em paralelo: se o git escrever muito, o pipe não enche e trava.
    let read_all = |pipe: Option<Box<dyn Read + Send>>| {
        thread::spawn(move || {
            let mut buf = String::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_string(&mut buf);
            }
            buf
        })
    };
    let out_reader = read_all(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let err_reader = read_all(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() > GIT_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{TIMEOUT_PREFIX} (git {})", args.first().unwrap_or(&"")));
            }
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
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
    } else if s.contains(NO_REMOTE_BRANCH) {
        format!("Falha no git: {NO_REMOTE_BRANCH} main")
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
    match git(dir, &["fetch", "-q", "origin", "main"]) {
        Ok(_) => {
            // Passa a seguir o histórico do remoto sem mexer nos arquivos locais:
            // o próximo commit entra por cima dele (sem conflitos). `--mixed`
            // mantém no índice os arquivos que só existem no remoto.
            git(dir, &["reset", "-q", "--mixed", "FETCH_HEAD"])?;
            Ok(())
        }
        // Repositório recém-criado no GitHub ainda não tem a branch main.
        Err(e) if e.contains(NO_REMOTE_BRANCH) => Ok(()),
        Err(e) => Err(e),
    }
}

fn has_identity(dir: &Path) -> bool {
    let set = |key| git(dir, &["config", key]).map(|s| !s.trim().is_empty()).unwrap_or(false);
    set("user.email") && set("user.name")
}

fn commit_and_push(dir: &Path) -> Result<bool, String> {
    // Só os arquivos do backup: qualquer outro arquivo do repositório fica intacto.
    let files: Vec<&str> = [DUMP_FILE, "README.md"].into_iter().filter(|f| dir.join(f).exists()).collect();
    if files.is_empty() {
        return Err("Nada para enviar".into());
    }
    let mut add = vec!["add", "--"];
    add.extend(files);
    git(dir, &add)?;
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

/// Prepara o repositório local. Se o endereço HTTPS falhar por falta de login,
/// tenta o mesmo repositório via SSH (a chave costuma já estar configurada) e,
/// se der certo, passa a usar o SSH.
fn connect(conn: &Shared, dir: &Path, url: &str) -> Result<(), String> {
    match prepare_repo(dir, url) {
        Ok(()) => Ok(()),
        Err(e) if is_auth_error(&e) => {
            let ssh = https_to_ssh(url).ok_or(e.clone())?;
            prepare_repo(dir, &ssh).map_err(|_| e)?;
            let c = conn.lock().unwrap_or_else(|e| e.into_inner());
            set_setting(&c, KEY_REPO, Some(&ssh));
            log(&format!("Sync: HTTPS sem login; passando a usar {ssh}"));
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Executa um envio completo. Devolve uma mensagem curta de resultado.
pub fn run(conn: &Shared, data_dir: &Path, app: Option<&AppHandle>) -> Result<String, String> {
    let Some(guard) = RunningGuard::acquire() else {
        return Err(BUSY.into());
    };
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
        connect(conn, &dir, &url)?;
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
    drop(guard);
    match &result {
        Ok(m) => log(&format!("Sync: {m}")),
        Err(e) => log(&format!("Sync falhou: {e}")),
    }
    notify(app, conn);
    result
}

// ---------------------------------------------------------------- restauração

/// Baixa o último `taskforme.sql` do repositório remoto.
fn fetch_remote_dump(dir: &Path) -> Result<String, String> {
    match git(dir, &["fetch", "-q", "origin", "main"]) {
        Ok(_) => {}
        Err(e) if e.contains(NO_REMOTE_BRANCH) => return Err("Ainda não há nenhum backup nesse repositório".into()),
        Err(e) => return Err(e),
    }
    git(dir, &["show", &format!("FETCH_HEAD:{DUMP_FILE}")])
        .map_err(|_| "O repositório não tem o arquivo taskforme.sql".to_string())
}

/// Confere se o SQL baixado é um backup íntegro do TaskForMe e o carrega em memória.
fn load_dump(sql: &str) -> Result<Connection, String> {
    let mem = Connection::open_in_memory().map_err(|e| e.to_string())?;
    mem.execute_batch(sql).map_err(|e| format!("O backup está danificado: {e}"))?;
    let version: i64 = mem.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if version < 1 || version as usize > crate::db::schema_version() {
        return Err("Esse backup é de uma versão mais nova do TaskForMe. Atualize o app antes de restaurar.".into());
    }
    let inbox: i64 = mem
        .query_row("SELECT COUNT(*) FROM projects WHERE is_inbox = 1", [], |r| r.get(0))
        .map_err(|_| "O arquivo não parece um backup do TaskForMe".to_string())?;
    if inbox == 0 {
        return Err("O arquivo não parece um backup do TaskForMe".into());
    }
    Ok(mem)
}

/// Substitui os dados atuais pelos do backup em memória. Antes, guarda uma
/// cópia local (`pre-restore`) e preserva as configurações do próprio backup.
fn replace_live(live: &mut Connection, mem: &Connection, data_dir: &Path) -> Result<(), String> {
    let keep: Vec<(String, String)> = live
        .prepare("SELECT key, value FROM settings WHERE key LIKE 'sync%'")
        .and_then(|mut st| st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .map_err(|e| e.to_string())?;

    crate::db::backup_now(live, data_dir, "pre-restore")
        .map_err(|e| format!("Não foi possível fazer a cópia de segurança antes de restaurar: {e}"))?;

    {
        let backup = rusqlite::backup::Backup::new(mem, live).map_err(|e| e.to_string())?;
        backup
            .run_to_completion(256, Duration::from_millis(0), None)
            .map_err(|e| format!("Falha ao restaurar: {e}"))?;
    }
    crate::db::migrate_open(live, data_dir)?;
    for (k, v) in keep {
        set_setting(live, &k, Some(&v));
    }
    Ok(())
}

/// Restaura os dados a partir do último backup no repositório configurado.
pub fn restore(conn: &Shared, data_dir: &Path) -> Result<String, String> {
    let result = restore_inner(conn, data_dir);
    if let Err(e) = &result {
        log(&format!("Sync: restauração falhou: {e}"));
    }
    result
}

fn restore_inner(conn: &Shared, data_dir: &Path) -> Result<String, String> {
    let Some(_guard) = RunningGuard::acquire() else {
        return Err(BUSY.into());
    };
    let url = {
        let c = conn.lock().unwrap_or_else(|e| e.into_inner());
        let url = get_setting(&c, KEY_REPO).ok_or_else(|| "Configure o endereço do repositório".to_string())?;
        validate_repo(&url)?
    };
    let dir = data_dir.join(REPO_DIR);
    connect(conn, &dir, &url)?;
    let sql = fetch_remote_dump(&dir)?;
    let mem = load_dump(&sql)?;

    let mut live = conn.lock().unwrap_or_else(|e| e.into_inner());
    replace_live(&mut live, &mem, data_dir)?;
    let projects: i64 = live
        .query_row("SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL AND is_inbox = 0", [], |r| r.get(0))
        .unwrap_or(0);
    log("Sync: dados restaurados do repositório");
    Ok(format!(
        "Dados restaurados do GitHub ({projects} {}). Uma cópia do estado anterior ficou nos backups locais.",
        if projects == 1 { "projeto" } else { "projetos" }
    ))
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
    fn nao_apaga_outros_arquivos_do_repositorio() {
        let base = std::env::temp_dir().join(format!(
            "taskforme-git-extra-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let remote = base.join("remote.git");
        fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "-q", "--bare"]).unwrap();
        let url = remote.to_string_lossy().to_string();

        // Alguém colocou um arquivo próprio no repositório de backup.
        let other = base.join("other");
        prepare_repo(&other, &url).unwrap();
        fs::write(other.join("minhas-anotacoes.txt"), "importante").unwrap();
        git(&other, &["add", "minhas-anotacoes.txt"]).unwrap();
        git(&other, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "x"]).unwrap();
        git(&other, &["push", "-q", "origin", "HEAD:refs/heads/main"]).unwrap();

        // O app, numa cópia nova (ex.: reinstalado), envia o backup.
        let local = base.join("local");
        prepare_repo(&local, &url).unwrap();
        fs::write(local.join(DUMP_FILE), "dados").unwrap();
        commit_and_push(&local).unwrap();

        let files = git(&remote, &["ls-tree", "--name-only", "main"]).unwrap();
        assert!(files.contains("minhas-anotacoes.txt"), "arquivo do usuário preservado: {files}");
        assert!(files.contains(DUMP_FILE));
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn restaura_do_repositorio_preservando_config_do_backup() {
        let base = std::env::temp_dir().join(format!(
            "taskforme-restore-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let remote = base.join("remote.git");
        fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "-q", "--bare"]).unwrap();
        let url = remote.to_string_lossy().to_string();

        // Computador A: tem um projeto e envia o backup.
        let a_dir = base.join("a");
        fs::create_dir_all(&a_dir).unwrap();
        let a = crate::db::open(&a_dir).unwrap().conn;
        a.execute("INSERT INTO projects (name, created_at) VALUES ('Projeto do A', 'x')", []).unwrap();
        let repo_a = a_dir.join(REPO_DIR);
        prepare_repo(&repo_a, &url).unwrap();
        fs::write(repo_a.join(DUMP_FILE), dump(&a).unwrap()).unwrap();
        commit_and_push(&repo_a).unwrap();

        // Computador B: dados diferentes e o backup configurado.
        let b_dir = base.join("b");
        fs::create_dir_all(&b_dir).unwrap();
        let mut b = crate::db::open(&b_dir).unwrap().conn;
        b.execute("INSERT INTO projects (name, created_at) VALUES ('Projeto do B', 'x')", []).unwrap();
        set_setting(&b, KEY_REPO, Some(&url));
        set_setting(&b, KEY_ENABLED, Some("true"));

        let repo_b = b_dir.join(REPO_DIR);
        prepare_repo(&repo_b, &url).unwrap();
        let mem = load_dump(&fetch_remote_dump(&repo_b).unwrap()).unwrap();
        replace_live(&mut b, &mem, &b_dir).unwrap();

        let names: Vec<String> = b
            .prepare("SELECT name FROM projects WHERE is_inbox = 0")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(names, vec!["Projeto do A".to_string()]);
        assert_eq!(get_setting(&b, KEY_REPO).as_deref(), Some(url.as_str()), "config do backup mantida");
        let pre = fs::read_dir(b_dir.join("backups"))
            .unwrap()
            .any(|e| e.unwrap().file_name().to_string_lossy().contains("pre-restore"));
        assert!(pre, "cópia de segurança antes de restaurar");
        // O banco continua íntegro e utilizável depois da troca.
        let ok: String = b.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
        assert_eq!(ok, "ok");
        b.execute("INSERT INTO projects (name, created_at) VALUES ('Novo', 'x')", []).unwrap();
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn backup_de_versao_antiga_restaura_na_versao_nova() {
        // Backup feito por uma versão anterior do app (sem a repetição semanal).
        let old = Connection::open_in_memory().unwrap();
        for sql in &crate::db::tests_support::MIGRATIONS_FOR_TESTS[..3] {
            old.execute_batch(sql).unwrap();
        }
        old.pragma_update(None, "user_version", 3).unwrap();
        old.execute(
            "INSERT INTO tasks (project_id, title, created_at, updated_at) VALUES (1, 'Do backup antigo', 'x', 'x')",
            [],
        )
        .unwrap();
        let sql = dump(&old).unwrap();

        let dir = std::env::temp_dir().join(format!(
            "taskforme-oldbackup-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let mut live = crate::db::open(&dir).unwrap().conn;
        replace_live(&mut live, &load_dump(&sql).unwrap(), &dir).unwrap();

        let v: i64 = live.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v as usize, crate::db::schema_version(), "atualizado para o formato novo");
        let title: String = live.query_row("SELECT title FROM tasks", [], |r| r.get(0)).unwrap();
        assert_eq!(title, "Do backup antigo");
        // E a coluna nova já funciona.
        live.execute("UPDATE tasks SET repeat_days = '1'", []).unwrap();
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn recusa_backup_invalido() {
        assert!(load_dump("isso não é sql").is_err());
        assert!(load_dump("CREATE TABLE x (a); PRAGMA user_version=1;").is_err());
        assert!(load_dump("CREATE TABLE projects (is_inbox INTEGER); PRAGMA user_version=99;").is_err());
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
