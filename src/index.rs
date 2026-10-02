use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde_json::json;

use crate::embed;
use crate::error::{HippoError, Result};
use crate::store::Memory;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS memories (
    id            TEXT PRIMARY KEY,
    path          TEXT NOT NULL,
    title         TEXT NOT NULL,
    keywords      TEXT NOT NULL,
    tags          TEXT,
    scope         TEXT NOT NULL DEFAULT 'global',
    updated       TEXT,
    content_hash  TEXT NOT NULL,
    hits          INTEGER NOT NULL DEFAULT 0,
    last_accessed TEXT,
    embedding     TEXT
);
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    title, keywords, body, id UNINDEXED,
    tokenize = 'porter unicode61'
);
";

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "hits",
        "ALTER TABLE memories ADD COLUMN hits INTEGER NOT NULL DEFAULT 0",
    ),
    (
        "last_accessed",
        "ALTER TABLE memories ADD COLUMN last_accessed TEXT",
    ),
    (
        "embedding",
        "ALTER TABLE memories ADD COLUMN embedding TEXT",
    ),
];

#[derive(Debug, Default)]
pub struct ReindexResult {
    pub added: i64,
    pub updated: i64,
    pub deleted: i64,
}

impl ReindexResult {
    pub fn total(&self) -> i64 {
        self.added + self.updated
    }
}

pub fn db_path(home: &Path) -> PathBuf {
    home.join(".index").join("memory.db")
}

pub fn connect(home: &Path) -> Result<Connection> {
    let path = db_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(HippoError::unexpected)?;
    }
    let conn = Connection::open(path).map_err(HippoError::unexpected)?;
    conn.execute_batch(SCHEMA).map_err(HippoError::unexpected)?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(memories)")
        .map_err(HippoError::unexpected)?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(HippoError::unexpected)?
        .collect::<std::result::Result<_, _>>()
        .map_err(HippoError::unexpected)?;
    for (column, ddl) in MIGRATIONS {
        if !existing.iter().any(|name| name == column) {
            conn.execute(ddl, []).map_err(HippoError::unexpected)?;
        }
    }
    Ok(())
}

pub fn stored_hashes(conn: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = conn
        .prepare("SELECT id, content_hash FROM memories")
        .map_err(HippoError::unexpected)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(HippoError::unexpected)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(HippoError::unexpected)
}

pub fn upsert(conn: &Connection, memory: &Memory) -> Result<()> {
    let (hits, last_accessed) = usage(conn, &memory.id)?;
    conn.execute("DELETE FROM memories WHERE id = ?1", [&memory.id])
        .map_err(HippoError::unexpected)?;
    conn.execute("DELETE FROM memories_fts WHERE id = ?1", [&memory.id])
        .map_err(HippoError::unexpected)?;
    conn.execute(
        "INSERT INTO memories (id, path, title, keywords, tags, scope, updated, content_hash, hits, last_accessed, embedding)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            memory.id,
            memory.path.display().to_string(),
            memory.title,
            memory.keywords.join(" "),
            memory.tags.join(" "),
            memory.scope,
            memory.updated,
            memory.content_hash,
            hits,
            last_accessed,
            embedding_json(memory),
        ],
    )
    .map_err(HippoError::unexpected)?;
    conn.execute(
        "INSERT INTO memories_fts (title, keywords, body, id) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            memory.title,
            memory.keywords.join(" "),
            memory.body,
            memory.id,
        ],
    )
    .map_err(HippoError::unexpected)?;
    Ok(())
}

pub fn delete(conn: &Connection, memory_id: &str) -> Result<()> {
    conn.execute("DELETE FROM memories WHERE id = ?1", [memory_id])
        .map_err(HippoError::unexpected)?;
    conn.execute("DELETE FROM memories_fts WHERE id = ?1", [memory_id])
        .map_err(HippoError::unexpected)?;
    Ok(())
}

pub fn record_access(conn: &Connection, ids: &[String], when: &str) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    for memory_id in ids {
        conn.execute(
            "UPDATE memories SET hits = hits + 1, last_accessed = ?1 WHERE id = ?2",
            rusqlite::params![when, memory_id],
        )
        .map_err(HippoError::unexpected)?;
    }
    Ok(())
}

pub fn hit_counts(conn: &Connection) -> Result<HashMap<String, i64>> {
    let mut stmt = conn
        .prepare("SELECT id, hits FROM memories")
        .map_err(HippoError::unexpected)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(HippoError::unexpected)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(HippoError::unexpected)
}

fn usage(conn: &Connection, memory_id: &str) -> Result<(i64, Option<String>)> {
    conn.query_row(
        "SELECT hits, last_accessed FROM memories WHERE id = ?1",
        [memory_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(HippoError::unexpected)
    .map(|row| row.unwrap_or((0, None)))
}

fn embedding_json(memory: &Memory) -> String {
    let text = format!(
        "{} {} {}",
        memory.title,
        memory.keywords.join(" "),
        memory.body
    );
    json!(embed::embed(&text)).to_string()
}
