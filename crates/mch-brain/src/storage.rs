//! Storage layer: SQLite (FTS5 + Vector similarity)

use crate::types::{
    ActionMetadata, ActionOutcome, CompositeRecord, MemoryRecord, MemoryTree, MemoryType, RecordId,
    SessionId, UsageStats,
};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Storage configuration
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// SQLite database path
    pub sqlite_path: String,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Enable FTS5
    pub enable_fts: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            sqlite_path: "data/brain.sqlite".to_string(),
            embedding_dim: 1536, // text-embedding-3-small
            enable_fts: true,
        }
    }
}

/// Brain storage trait for abstraction
#[async_trait]
pub trait BrainStorage: Send + Sync {
    async fn init(&self) -> Result<()>;
    async fn store_record(&self, record: &MemoryRecord) -> Result<()>;
    async fn get_record(&self, id: &RecordId) -> Result<Option<MemoryRecord>>;
    async fn delete_record(&self, id: &RecordId) -> Result<()>;
    async fn search_fts(&self, query: &str, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryRecord>>;
    async fn vector_search(&self, embedding: &[f32], memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryRecord>>;
    async fn store_composite(&self, composite: &CompositeRecord) -> Result<()>;
    async fn get_composite(&self, id: &RecordId) -> Result<Option<CompositeRecord>>;
    async fn list_composites(&self, memory_type: Option<MemoryType>) -> Result<Vec<CompositeRecord>>;
    async fn update_usage(&self, id: &RecordId, outcome: Option<ActionOutcome>) -> Result<()>;
    async fn get_tree(&self) -> Result<MemoryTree>;
}

/// SQLite store for FTS5 full-text search, relational data, and vector similarity
pub struct SqliteStore {
    pool: SqlitePool,
    config: StorageConfig,
}

impl SqliteStore {
    pub async fn new(config: StorageConfig) -> Result<Self> {
        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(&config.sqlite_path).parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&format!("sqlite://{}?mode=rwc", config.sqlite_path))
            .await?;

        let store = Self { pool, config };
        store.init().await?;
        Ok(store)
    }

    async fn init(&self) -> Result<()> {
        // Memory records table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS memory_records (
                record_id TEXT PRIMARY KEY,
                memory_type TEXT NOT NULL,
                normalized_text TEXT NOT NULL,
                semantic_text TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                usage_json TEXT NOT NULL,
                session_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                child_composite_ids TEXT NOT NULL,
                source_record_ids TEXT NOT NULL,
                embedding BLOB
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Composite records table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS composite_records (
                record_id TEXT PRIMARY KEY,
                memory_type TEXT NOT NULL,
                semantic_text TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                usage_json TEXT NOT NULL,
                session_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                child_composite_ids TEXT NOT NULL,
                source_record_ids TEXT NOT NULL,
                child_composite_ids_level2 TEXT NOT NULL,
                embedding BLOB
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // FTS5 virtual table for full-text search
        if self.config.enable_fts {
            sqlx::query(
                r#"
                CREATE VIRTUAL TABLE IF NOT EXISTS memory_records_fts USING fts5(
                    record_id UNINDEXED,
                    memory_type,
                    normalized_text,
                    semantic_text,
                    content='memory_records',
                    content_rowid='rowid'
                )
                "#,
            )
            .execute(&self.pool)
            .await?;

            // Triggers to keep FTS in sync
            sqlx::query(
                r#"
                CREATE TRIGGER IF NOT EXISTS memory_records_ai AFTER INSERT ON memory_records BEGIN
                    INSERT INTO memory_records_fts(rowid, record_id, memory_type, normalized_text, semantic_text)
                    VALUES (new.rowid, new.record_id, new.memory_type, new.normalized_text, new.semantic_text);
                END
                "#,
            )
            .execute(&self.pool)
            .await?;

            sqlx::query(
                r#"
                CREATE TRIGGER IF NOT EXISTS memory_records_ad AFTER DELETE ON memory_records BEGIN
                    INSERT INTO memory_records_fts(memory_records_fts, rowid, record_id, memory_type, normalized_text, semantic_text)
                    VALUES ('delete', old.rowid, old.record_id, old.memory_type, old.normalized_text, old.semantic_text);
                END
                "#,
            )
            .execute(&self.pool)
            .await?;

            sqlx::query(
                r#"
                CREATE TRIGGER IF NOT EXISTS memory_records_au AFTER UPDATE ON memory_records BEGIN
                    INSERT INTO memory_records_fts(memory_records_fts, rowid, record_id, memory_type, normalized_text, semantic_text)
                    VALUES ('delete', old.rowid, old.record_id, old.memory_type, old.normalized_text, old.semantic_text);
                    INSERT INTO memory_records_fts(rowid, record_id, memory_type, normalized_text, semantic_text)
                    VALUES (new.rowid, new.record_id, new.memory_type, new.normalized_text, new.semantic_text);
                END
                "#,
            )
            .execute(&self.pool)
            .await?;
        }

        // Indexes
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_memory_type ON memory_records(memory_type)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_session ON memory_records(session_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_composite_type ON composite_records(memory_type)")
            .execute(&self.pool)
            .await?;

        info!("SQLite store initialized at {}", self.config.sqlite_path);
        Ok(())
    }
}

#[async_trait]
impl BrainStorage for SqliteStore {
    async fn init(&self) -> Result<()> {
        // Already done in new()
        Ok(())
    }

    async fn store_record(&self, record: &MemoryRecord) -> Result<()> {
        let metadata_json = serde_json::to_string(&record.metadata)?;
        let usage_json = serde_json::to_string(&record.usage)?;
        let child_ids = serde_json::to_string(&record.child_composite_ids)?;
        let source_ids = serde_json::to_string(&record.source_record_ids)?;
        let embedding = record.embedding.as_ref().map(|e| {
            e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>()
        });

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO memory_records 
            (record_id, memory_type, normalized_text, semantic_text, metadata_json, usage_json,
             session_id, created_at, updated_at, child_composite_ids, source_record_ids, embedding)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&record.record_id.0)
        .bind(record.memory_type.to_string())
        .bind(&record.normalized_text)
        .bind(&record.semantic_text)
        .bind(&metadata_json)
        .bind(&usage_json)
        .bind(record.session_id.to_string())
        .bind(record.created_at.to_rfc3339())
        .bind(record.updated_at.to_rfc3339())
        .bind(&child_ids)
        .bind(&source_ids)
        .bind(embedding)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_record(&self, id: &RecordId) -> Result<Option<MemoryRecord>> {
        let row = sqlx::query(
            "SELECT * FROM memory_records WHERE record_id = ?"
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(row_to_record(row)?))
        } else {
            Ok(None)
        }
    }

    async fn delete_record(&self, id: &RecordId) -> Result<()> {
        sqlx::query("DELETE FROM memory_records WHERE record_id = ?")
            .bind(&id.0)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn search_fts(&self, query: &str, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryRecord>> {
        let rows = if let Some(mt) = memory_type {
            sqlx::query(
                "SELECT m.* FROM memory_records m 
                 JOIN memory_records_fts f ON m.rowid = f.rowid 
                 WHERE memory_records_fts MATCH ? AND m.memory_type = ?
                 ORDER BY rank LIMIT ?"
            )
            .bind(query.to_string())
            .bind(mt.to_string())
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT m.* FROM memory_records m 
                 JOIN memory_records_fts f ON m.rowid = f.rowid 
                 WHERE memory_records_fts MATCH ?
                 ORDER BY rank LIMIT ?"
            )
            .bind(query.to_string())
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(rows.into_iter().filter_map(|r| row_to_record(r).ok()).collect())
    }

    async fn vector_search(&self, embedding: &[f32], memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryRecord>> {
        // Simple cosine similarity search in SQLite
        // Fetch all records with embeddings, compute similarity in memory
        let rows = if let Some(mt) = memory_type {
            sqlx::query("SELECT * FROM memory_records WHERE embedding IS NOT NULL AND memory_type = ?")
                .bind(mt.to_string())
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query("SELECT * FROM memory_records WHERE embedding IS NOT NULL")
                .fetch_all(&self.pool)
                .await?
        };
        
        // Compute cosine similarities
        let mut scored = Vec::new();
        for row in rows {
            if let Ok(record) = row_to_record(row) {
                if let Some(ref stored_embedding) = record.embedding {
                    let sim = cosine_similarity(embedding, stored_embedding);
                    scored.push((sim, record));
                }
            }
        }
        
        // Sort by similarity descending
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(scored.into_iter().take(limit).map(|(_, r)| r).collect())
    }

    async fn store_composite(&self, composite: &CompositeRecord) -> Result<()> {
        let metadata_json = serde_json::to_string(&composite.metadata)?;
        let usage_json = serde_json::to_string(&composite.usage)?;
        let child_ids = serde_json::to_string(&composite.child_composite_ids)?;
        let source_ids = serde_json::to_string(&composite.source_record_ids)?;
        let child_ids_l2 = serde_json::to_string(&composite.child_composite_ids_level2)?;
        let embedding = composite.embedding.as_ref().map(|e| {
            e.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>()
        });

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO composite_records 
            (record_id, memory_type, semantic_text, metadata_json, usage_json,
             session_id, created_at, updated_at, child_composite_ids, source_record_ids, 
             child_composite_ids_level2, embedding)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&composite.record_id.0)
        .bind(composite.memory_type.to_string())
        .bind(&composite.semantic_text)
        .bind(&metadata_json)
        .bind(&usage_json)
        .bind(composite.session_id.to_string())
        .bind(composite.created_at.to_rfc3339())
        .bind(composite.updated_at.to_rfc3339())
        .bind(&child_ids)
        .bind(&source_ids)
        .bind(&child_ids_l2)
        .bind(embedding)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_composite(&self, id: &RecordId) -> Result<Option<CompositeRecord>> {
        let row = sqlx::query("SELECT * FROM composite_records WHERE record_id = ?")
            .bind(&id.0)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(row) = row {
            Ok(Some(row_to_composite(row)?))
        } else {
            Ok(None)
        }
    }

    async fn list_composites(&self, memory_type: Option<MemoryType>) -> Result<Vec<CompositeRecord>> {
        let mut sql = String::from("SELECT * FROM composite_records");
        if let Some(mt) = memory_type {
            sql.push_str(" WHERE memory_type = ?");
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut q = sqlx::query(&sql);
        if let Some(mt) = memory_type {
            q = q.bind(mt.to_string());
        }

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().filter_map(|r| row_to_composite(r).ok()).collect())
    }

    async fn update_usage(&self, id: &RecordId, outcome: Option<ActionOutcome>) -> Result<()> {
        let record = self.get_record(id).await?;
        if let Some(mut record) = record {
            record.usage.retrieval_count += 1;
            record.usage.last_retrieved = Some(Utc::now());
            if let Some(ref o) = outcome {
                record.usage.last_outcome = Some(o.clone());
                match o {
                    ActionOutcome::Success => record.usage.action_success_count += 1,
                    ActionOutcome::Failure => record.usage.action_failure_count += 1,
                    ActionOutcome::Correction => {}
                }
            }
            record.updated_at = Utc::now();
            self.store_record(&record).await?;
        }
        Ok(())
    }

    async fn get_tree(&self) -> Result<MemoryTree> {
        let composites = self.list_composites(None).await?;
        let atomic_rows = sqlx::query("SELECT * FROM memory_records")
            .fetch_all(&self.pool)
            .await?;

        let mut tree = MemoryTree::default();
        for composite in composites {
            tree.composites.insert(composite.record_id.clone(), composite.clone());
            if composite.child_composite_ids_level2.is_empty() {
                tree.roots.push(composite.record_id);
            } else {
                tree.level2.push(composite.record_id);
            }
        }

        for row in atomic_rows {
            if let Ok(record) = row_to_record(row) {
                tree.atomic.insert(record.record_id.clone(), record);
            }
        }

        Ok(tree)
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Convert SQLx row to MemoryRecord
fn row_to_record(row: sqlx::sqlite::SqliteRow) -> Result<MemoryRecord> {
    let metadata_json: String = row.try_get("metadata_json")?;
    let usage_json: String = row.try_get("usage_json")?;
    let child_ids_json: String = row.try_get("child_composite_ids")?;
    let source_ids_json: String = row.try_get("source_record_ids")?;
    let embedding_blob: Option<Vec<u8>> = row.try_get("embedding")?;

    let embedding = embedding_blob.as_ref().map(|blob| {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<f32>>()
    });

    let record = MemoryRecord {
        record_id: RecordId(row.try_get("record_id")?),
        memory_type: serde_json::from_str(&format!("\"{}\"", row.try_get::<String, _>("memory_type")?))?,
        normalized_text: row.try_get("normalized_text")?,
        semantic_text: row.try_get("semantic_text")?,
        metadata: serde_json::from_str(&metadata_json)?,
        usage: serde_json::from_str(&usage_json)?,
        session_id: SessionId(uuid::Uuid::parse_str(&row.try_get::<String, _>("session_id")?)?),
        created_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("created_at")?)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("updated_at")?)?.with_timezone(&Utc),
        child_composite_ids: serde_json::from_str(&child_ids_json)?,
        source_record_ids: serde_json::from_str(&source_ids_json)?,
        embedding,
    };
    Ok(record)
}

/// Convert SQLx row to CompositeRecord
fn row_to_composite(row: sqlx::sqlite::SqliteRow) -> Result<CompositeRecord> {
    let metadata_json: String = row.try_get("metadata_json")?;
    let usage_json: String = row.try_get("usage_json")?;
    let child_ids_json: String = row.try_get("child_composite_ids")?;
    let source_ids_json: String = row.try_get("source_record_ids")?;
    let child_ids_l2_json: String = row.try_get("child_composite_ids_level2")?;
    let embedding_blob: Option<Vec<u8>> = row.try_get("embedding")?;

    let embedding = embedding_blob.as_ref().map(|blob| {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect::<Vec<f32>>()
    });

    let composite = CompositeRecord {
        record_id: RecordId(row.try_get("record_id")?),
        memory_type: serde_json::from_str(&format!("\"{}\"", row.try_get::<String, _>("memory_type")?))?,
        semantic_text: row.try_get("semantic_text")?,
        metadata: serde_json::from_str(&metadata_json)?,
        usage: serde_json::from_str(&usage_json)?,
        session_id: SessionId(uuid::Uuid::parse_str(&row.try_get::<String, _>("session_id")?)?),
        created_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("created_at")?)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&row.try_get::<String, _>("updated_at")?)?.with_timezone(&Utc),
        child_composite_ids: serde_json::from_str(&child_ids_json)?,
        source_record_ids: serde_json::from_str(&source_ids_json)?,
        child_composite_ids_level2: serde_json::from_str(&child_ids_l2_json)?,
        embedding,
    };
    Ok(composite)
}

/// Unified store that combines SQLite for everything
pub struct UnifiedStore {
    sqlite: SqliteStore,
}

impl UnifiedStore {
    pub async fn new(config: StorageConfig) -> Result<Self> {
        let sqlite = SqliteStore::new(config).await?;
        Ok(Self { sqlite })
    }
}

#[async_trait]
impl BrainStorage for UnifiedStore {
    async fn init(&self) -> Result<()> {
        self.sqlite.init().await
    }

    async fn store_record(&self, record: &MemoryRecord) -> Result<()> {
        self.sqlite.store_record(record).await
    }

    async fn get_record(&self, id: &RecordId) -> Result<Option<MemoryRecord>> {
        self.sqlite.get_record(id).await
    }

    async fn delete_record(&self, id: &RecordId) -> Result<()> {
        self.sqlite.delete_record(id).await
    }

    async fn search_fts(&self, query: &str, memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.sqlite.search_fts(query, memory_type, limit).await
    }

    async fn vector_search(&self, embedding: &[f32], memory_type: Option<MemoryType>, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.sqlite.vector_search(embedding, memory_type, limit).await
    }

    async fn store_composite(&self, composite: &CompositeRecord) -> Result<()> {
        self.sqlite.store_composite(composite).await
    }

    async fn get_composite(&self, id: &RecordId) -> Result<Option<CompositeRecord>> {
        self.sqlite.get_composite(id).await
    }

    async fn list_composites(&self, memory_type: Option<MemoryType>) -> Result<Vec<CompositeRecord>> {
        self.sqlite.list_composites(memory_type).await
    }

    async fn update_usage(&self, id: &RecordId, outcome: Option<ActionOutcome>) -> Result<()> {
        self.sqlite.update_usage(id, outcome).await
    }

    async fn get_tree(&self) -> Result<MemoryTree> {
        self.sqlite.get_tree().await
    }
}