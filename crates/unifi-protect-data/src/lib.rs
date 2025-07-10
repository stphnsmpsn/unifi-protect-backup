use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool, migrate::MigrateDatabase, sqlite::SqlitePoolOptions};

pub mod error;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    pub camera_id: String,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub backed_up: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub event_id: String,
    pub remote_path: String,
    pub backup_time: DateTime<Utc>,
    pub size_bytes: u64,
}

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(db_path: &Path) -> Result<Self> {
        if !sqlx::Sqlite::database_exists(&db_path.to_string_lossy()).await? {
            sqlx::Sqlite::create_database(&db_path.to_string_lossy()).await?;
        }

        let database_url = format!("sqlite:{}", db_path.display());

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Database { pool })
    }

    pub async fn insert_event(&self, event: &Event) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO events (id, event_type, camera_id, start_time, end_time, backed_up)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
            event.id,
            event.event_type,
            event.camera_id,
            event.start_time,
            event.end_time,
            event.backed_up
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_event_backed_up(&self, event_id: &str) -> Result<()> {
        sqlx::query!("UPDATE events SET backed_up = TRUE WHERE id = ?", event_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn insert_backup(&self, backup: &Backup) -> Result<()> {
        let size_bytes = backup.size_bytes as i64;
        let timestamp = backup.backup_time.timestamp();
        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO backups (event_id, remote_path, backup_time, size_bytes)
            VALUES (?, ?, ?, ?)
            "#,
            backup.event_id,
            backup.remote_path,
            timestamp,
            size_bytes
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_event_by_id(&self, id: &str) -> Result<Option<Event>> {
        let event = sqlx::query_as!(
            Event,
            r#"
            SELECT id as "id!: String", 
                   event_type as "event_type!: _",
                   camera_id as "camera_id!: _",
                   start_time as "start_time!: _",
                   end_time as "end_time?: _",
                   backed_up as "backed_up!: _"
            FROM events WHERE id = ?
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(event)
    }

    pub async fn get_events_not_backed_up(&self) -> Result<Vec<Event>> {
        let events = sqlx::query_as!(
            Event,
            r#"
            SELECT id as "id!: String", 
                   event_type as "event_type!: _",
                   camera_id as "camera_id!: _",
                   start_time as "start_time!: _",
                   end_time as "end_time?: _",
                   backed_up as "backed_up!: _"
            FROM events WHERE backed_up = FALSE AND end_time IS NOT NULL
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    pub async fn get_events_by_camera(&self, camera_id: &str) -> Result<Vec<Event>> {
        let events = sqlx::query_as!(
            Event,
            r#"
            SELECT id as "id!: String", 
                   event_type as "event_type!: _",
                   camera_id as "camera_id!: _",
                   start_time as "start_time!: _",
                   end_time as "end_time?: _",
                   backed_up as "backed_up!: _"
            FROM events WHERE camera_id = ?
            "#,
            camera_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(events)
    }

    pub async fn cleanup_old_events(&self, retention_period: u32) -> Result<()> {
        let cutoff_time =
            (Utc::now() - chrono::Duration::days(retention_period as i64)).timestamp();

        sqlx::query!("DELETE FROM events WHERE start_time < ?", cutoff_time)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
