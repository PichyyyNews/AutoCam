use rusqlite::{params, Connection, Result};
use std::path::Path;
use std::time::Instant;


#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub inserted_count: usize,
    pub elapsed_ms: u128,
    pub records_per_sec: f64,
    pub total_records: usize,
}

#[derive(Debug, Clone)]
pub struct DbRecord {
    pub id: i64,
    pub tag: String,
    pub value: f64,
    pub created_at: i64,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    /// Initialize SQLite with extreme performance PRAGMAs:
    /// - WAL (Write-Ahead Logging): non-blocking concurrent reads and fast writes
    /// - synchronous = NORMAL: safe with WAL, drastically cuts disk flush latency
    /// - temp_store = MEMORY: stores temp tables/indices in RAM
    /// - cache_size = -64000: allocates 64 MB of RAM cache
    /// - mmap_size = 256MB: enables direct memory-mapped file I/O
    pub fn open_optimized<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Extreme speed PRAGMAs
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -64000;
            PRAGMA mmap_size = 268435456;
            PRAGMA busy_timeout = 5000;
            PRAGMA foreign_keys = ON;
            ",
        )?;

        // Create initial schema
        conn.execute(
            "
            CREATE TABLE IF NOT EXISTS records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tag TEXT NOT NULL,
                value REAL NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_records_created ON records(created_at);
            ",
            [],
        )?;

        Ok(Self { conn })
    }

    /// High-performance bulk insert using a single transaction and prepared statement
    pub fn benchmark_insert(&mut self, count: usize) -> Result<BenchmarkResult> {
        let start = Instant::now();
        let now_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Wrap batch in a single transaction for maximum I/O throughput
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(
                "INSERT INTO records (tag, value, created_at) VALUES (?1, ?2, ?3)",
            )?;

            for i in 0..count {
                let tag = format!("SENSOR_{:03}", i % 100);
                let value = (i as f64 * 0.123) % 1000.0;
                stmt.execute(params![tag, value, now_ts])?;
            }
        }
        tx.commit()?;

        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis().max(1);
        let records_per_sec = (count as f64) / elapsed.as_secs_f64();

        let total_records: usize = self
            .conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| {
                let cnt: i64 = r.get(0)?;
                Ok(cnt as usize)
            })?;

        Ok(BenchmarkResult {
            inserted_count: count,
            elapsed_ms,
            records_per_sec,
            total_records,
        })
    }

    /// Fetch latest records for UI display
    pub fn fetch_latest(&self, limit: usize) -> Result<Vec<DbRecord>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT id, tag, value, created_at FROM records ORDER BY id DESC LIMIT ?1",
        )?;

        let rows = stmt.query_map([limit as i64], |row| {
            Ok(DbRecord {
                id: row.get(0)?,
                tag: row.get(1)?,
                value: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;

        let mut records = Vec::new();
        for r in rows {
            records.push(r?);
        }
        Ok(records)
    }

    pub fn count_total(&self) -> Result<usize> {
        self.conn
            .query_row("SELECT COUNT(*) FROM records", [], |r| {
                let cnt: i64 = r.get(0)?;
                Ok(cnt as usize)
            })
    }
}
