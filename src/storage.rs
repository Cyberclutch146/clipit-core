use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use std::path::PathBuf;

pub type DbPool = Pool<SqliteConnectionManager>;

pub struct StorageEngine {
    pool: DbPool,
}

impl StorageEngine {
    pub fn new(db_path: PathBuf) -> Result<Self, String> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::new(manager).map_err(|e| e.to_string())?;

        let engine = Self { pool };
        engine.init_schema()?;

        Ok(engine)
    }

    fn init_schema(&self) -> Result<(), String> {
        let conn = self.pool.get().map_err(|e| e.to_string())?;

        // Enable WAL mode for better concurrency
        conn.execute_batch("PRAGMA journal_mode = WAL;").map_err(|e| e.to_string())?;

        // 1. Create clipboard items table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS clipboard_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type TEXT NOT NULL,
                text_content TEXT,
                blob_content BLOB,
                source_app TEXT,
                source_title TEXT,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                pinned BOOLEAN DEFAULT 0
            )",
            [],
        ).map_err(|e| e.to_string())?;

        // 2. Create FTS5 virtual table for full-text search
        conn.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts USING fts5(
                text_content,
                source_app,
                source_title,
                content='clipboard_items',
                content_rowid='id'
            )",
            [],
        ).map_err(|e| e.to_string())?;

        // 3. Create triggers to keep FTS index updated
        conn.execute_batch(
            "
            CREATE TRIGGER IF NOT EXISTS clipboard_items_ai AFTER INSERT ON clipboard_items BEGIN
                INSERT INTO clipboard_fts(rowid, text_content, source_app, source_title)
                VALUES (new.id, new.text_content, new.source_app, new.source_title);
            END;
            CREATE TRIGGER IF NOT EXISTS clipboard_items_ad AFTER DELETE ON clipboard_items BEGIN
                INSERT INTO clipboard_fts(clipboard_fts, rowid, text_content, source_app, source_title)
                VALUES ('delete', old.id, old.text_content, old.source_app, old.source_title);
            END;
            CREATE TRIGGER IF NOT EXISTS clipboard_items_au AFTER UPDATE ON clipboard_items BEGIN
                INSERT INTO clipboard_fts(clipboard_fts, rowid, text_content, source_app, source_title)
                VALUES ('delete', old.id, old.text_content, old.source_app, old.source_title);
                INSERT INTO clipboard_fts(rowid, text_content, source_app, source_title)
                VALUES (new.id, new.text_content, new.source_app, new.source_title);
            END;
            "
        ).map_err(|e| e.to_string())?;

        // 4. Settings table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        ).map_err(|e| e.to_string())?;

        // 5. Exclusion rules table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS exclusion_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_type TEXT NOT NULL,
                pattern TEXT NOT NULL
            )",
            [],
        ).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn get_connection(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>, String> {
        self.pool.get().map_err(|e| e.to_string())
    }

    pub fn insert_item(&self, content_type: &str, text_content: Option<&str>, source_app: Option<&str>) -> Result<i64, String> {
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO clipboard_items (content_type, text_content, source_app) VALUES (?1, ?2, ?3)",
            rusqlite::params![content_type, text_content, source_app],
        ).map_err(|e| e.to_string())?;
        
        Ok(conn.last_insert_rowid())
    }

    pub fn get_items(&self, limit: u32, offset: u32) -> Result<Vec<clipit_types::ClipboardItem>, String> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, content_type, text_content, source_app, source_title, created_at, pinned 
             FROM clipboard_items 
             ORDER BY created_at DESC 
             LIMIT ?1 OFFSET ?2"
        ).map_err(|e| e.to_string())?;

        let iter = stmt.query_map(rusqlite::params![limit, offset], |row| {
            Ok(clipit_types::ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                source_app: row.get(3)?,
                source_title: row.get(4)?,
                created_at: row.get(5)?,
                pinned: row.get(6)?,
            })
        }).map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for item in iter {
            if let Ok(i) = item {
                items.push(i);
            }
        }
        Ok(items)
    }

    pub fn search_items(&self, query: &str, limit: u32, offset: u32) -> Result<Vec<clipit_types::ClipboardItem>, String> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT i.id, i.content_type, i.text_content, i.source_app, i.source_title, i.created_at, i.pinned 
             FROM clipboard_fts f
             JOIN clipboard_items i ON f.rowid = i.id
             WHERE clipboard_fts MATCH ?1
             ORDER BY i.created_at DESC 
             LIMIT ?2 OFFSET ?3"
        ).map_err(|e| e.to_string())?;

        // FTS5 syntax requires formatting, simple prefix match for MVP
        let fts_query = format!("*{}*", query);

        let iter = stmt.query_map(rusqlite::params![fts_query, limit, offset], |row| {
            Ok(clipit_types::ClipboardItem {
                id: row.get(0)?,
                content_type: row.get(1)?,
                text_content: row.get(2)?,
                source_app: row.get(3)?,
                source_title: row.get(4)?,
                created_at: row.get(5)?,
                pinned: row.get(6)?,
            })
        }).map_err(|e| e.to_string())?;

        let mut items = Vec::new();
        for item in iter {
            if let Ok(i) = item {
                items.push(i);
            }
        }
        Ok(items)
    }
}
