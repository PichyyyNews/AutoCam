use rusqlite::{params, Connection, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

static DB_MUTEX: Mutex<()> = Mutex::new(());

fn get_db_path() -> PathBuf {
    // 1. Portable mode: Next to exe if directory is writable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let candidate = parent.join("autocam.db");
            if candidate.exists() || std::fs::OpenOptions::new().write(true).create(true).open(&candidate).is_ok() {
                return candidate;
            }
        }
    }

    // 2. Standard Windows user AppData directory (%APPDATA%\AutoCam\autocam.db)
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("AutoCam");
        let _ = std::fs::create_dir_all(&dir);
        let candidate = dir.join("autocam.db");
        if candidate.exists() || std::fs::OpenOptions::new().write(true).create(true).open(&candidate).is_ok() {
            return candidate;
        }
    }

    // 3. System temp directory fallback (%TEMP%\AutoCam\autocam.db)
    let temp_dir = std::env::temp_dir().join("AutoCam");
    let _ = std::fs::create_dir_all(&temp_dir);
    temp_dir.join("autocam.db")
}

fn open_connection() -> Result<Connection> {
    let path = get_db_path();
    let conn = Connection::open(&path).or_else(|_| Connection::open_in_memory())?;

    // Fast WAL configuration
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA temp_store = MEMORY;
        PRAGMA busy_timeout = 3000;
        ",
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
        [],
    )?;

    Ok(conn)
}

/// Initialize SQLite database and ensure schema exists
pub fn init() {
    let _lock = DB_MUTEX.lock().unwrap();
    if let Err(e) = open_connection() {
        eprintln!("[AutoCam DB] Init error: {}", e);
    }
}

/// Load all persistent key-value settings from the database
pub fn load_all() -> HashMap<String, String> {
    let _lock = DB_MUTEX.lock().unwrap();
    let mut map = HashMap::new();
    if let Ok(conn) = open_connection() {
        if let Ok(mut stmt) = conn.prepare("SELECT key, value FROM app_settings") {
            let rows = stmt.query_map([], |row| {
                let k: String = row.get(0)?;
                let v: String = row.get(1)?;
                Ok((k, v))
            });
            if let Ok(entries) = rows {
                for entry in entries.flatten() {
                    map.insert(entry.0, entry.1);
                }
            }
        }
    }
    map
}

/// Save a single key-value setting into SQLite
pub fn set(key: &str, value: &str) {
    let _lock = DB_MUTEX.lock().unwrap();
    if let Ok(conn) = open_connection() {
        let _ = conn.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        );
    }
}

/// Save multiple settings in a single transaction
#[allow(dead_code)]
pub fn set_batch(pairs: &[(&str, &str)]) {
    let _lock = DB_MUTEX.lock().unwrap();
    if let Ok(mut conn) = open_connection() {
        if let Ok(tx) = conn.transaction() {
            {
                let mut stmt = match tx.prepare_cached(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                ) {
                    Ok(s) => s,
                    Err(_) => return,
                };
                for (k, v) in pairs {
                    let _ = stmt.execute(params![*k, *v]);
                }
            }
            let _ = tx.commit();
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_set_and_load() {
        init();
        set("test_key", "test_val_123");
        let map = load_all();
        assert_eq!(map.get("test_key"), Some(&"test_val_123".to_string()));
    }
}
