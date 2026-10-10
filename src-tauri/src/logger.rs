//! Log mínimo em arquivo. Nunca falha: se não conseguir escrever, ignora.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
const MAX_LOG_BYTES: u64 = 1_000_000;

pub fn init(dir: PathBuf) {
    let path = dir.join("taskforme.log");
    // Rotaciona quando passa de ~1 MB para o arquivo não crescer para sempre.
    if std::fs::metadata(&path).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, dir.join("taskforme.old.log"));
    }
    let _ = LOG_PATH.set(path);

    std::panic::set_hook(Box::new(|info| {
        log(&format!("PANIC: {info}"));
    }));
}

pub fn log(msg: &str) {
    let line = format!("[{}] {msg}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    eprint!("{line}");
    if let Some(path) = LOG_PATH.get() {
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
