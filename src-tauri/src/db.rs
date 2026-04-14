use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS transcriptions (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    raw_text TEXT NOT NULL,
    cleaned_text TEXT,
    audio_path TEXT,
    stt_model TEXT,
    llm_model TEXT,
    duration_ms INTEGER,
    meta TEXT
);

CREATE INDEX IF NOT EXISTS idx_transcriptions_created_at
    ON transcriptions(created_at DESC);

CREATE TABLE IF NOT EXISTS llm_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transcription_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(transcription_id) REFERENCES transcriptions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_llm_messages_tx
    ON llm_messages(transcription_id);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcription {
    pub id: String,
    pub created_at: String,
    pub raw_text: String,
    pub cleaned_text: Option<String>,
    pub audio_path: Option<String>,
    pub stt_model: Option<String>,
    pub llm_model: Option<String>,
    pub duration_ms: Option<i64>,
    pub meta: Option<String>,
}

pub struct Db {
    conn: rusqlite::Connection,
}

impl Db {
    pub fn open(path: PathBuf) -> crate::error::Result<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).ok();
        }
        let conn = rusqlite::Connection::open(path).map_err(|e| anyhow::anyhow!(e))?;
        conn.execute_batch(SCHEMA_SQL).map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self { conn })
    }

    pub fn insert_transcription(&self, t: &Transcription) -> crate::error::Result<()> {
        self.conn
            .execute(
                "INSERT INTO transcriptions \
                 (id, created_at, raw_text, cleaned_text, audio_path, stt_model, llm_model, duration_ms, meta) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    t.id,
                    t.created_at,
                    t.raw_text,
                    t.cleaned_text,
                    t.audio_path,
                    t.stt_model,
                    t.llm_model,
                    t.duration_ms,
                    t.meta,
                ],
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    pub fn insert_message(
        &self,
        tx_id: &str,
        role: &str,
        content: &str,
    ) -> crate::error::Result<()> {
        self.conn
            .execute(
                "INSERT INTO llm_messages (transcription_id, role, content, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![tx_id, role, content, Utc::now().to_rfc3339()],
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    pub fn list_recent(&self, limit: i64) -> crate::error::Result<Vec<Transcription>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, created_at, raw_text, cleaned_text, audio_path, \
                 stt_model, llm_model, duration_ms, meta \
                 FROM transcriptions ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| anyhow::anyhow!(e))?;
        let rows = stmt
            .query_map([limit], |r| {
                Ok(Transcription {
                    id: r.get(0)?,
                    created_at: r.get(1)?,
                    raw_text: r.get(2)?,
                    cleaned_text: r.get(3)?,
                    audio_path: r.get(4)?,
                    stt_model: r.get(5)?,
                    llm_model: r.get(6)?,
                    duration_ms: r.get(7)?,
                    meta: r.get(8)?,
                })
            })
            .map_err(|e| anyhow::anyhow!(e))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| anyhow::anyhow!(e))?);
        }
        Ok(out)
    }

    pub fn delete(&self, id: &str) -> crate::error::Result<()> {
        self.conn
            .execute("DELETE FROM transcriptions WHERE id = ?1", [id])
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }

    pub fn clear_all(&self) -> crate::error::Result<()> {
        self.conn
            .execute_batch("DELETE FROM transcriptions; DELETE FROM llm_messages;")
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(())
    }
}
