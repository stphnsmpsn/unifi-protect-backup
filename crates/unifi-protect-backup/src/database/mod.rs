use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, migrate::MigrateDatabase, sqlite::SqlitePoolOptions};

use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub event_type: String,
    pub camera_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
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

        // Create tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                camera_id TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL,
                backed_up BOOLEAN NOT NULL DEFAULT FALSE
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS backups (
                event_id TEXT NOT NULL,
                remote_path TEXT NOT NULL,
                backup_time INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                PRIMARY KEY (event_id, remote_path),
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&pool)
        .await?;

        // Enable foreign key constraints
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await?;

        Ok(Database { pool })
    }

    pub async fn insert_event(&self, event: &Event) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO events (id, event_type, camera_id, start_time, end_time, backed_up)
            VALUES (?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&event.id)
        .bind(&event.event_type)
        .bind(&event.camera_id)
        .bind(event.start_time.timestamp())
        .bind(event.end_time.timestamp())
        .bind(event.backed_up)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_event_backed_up(&self, event_id: &str) -> Result<()> {
        sqlx::query("UPDATE events SET backed_up = TRUE WHERE id = ?")
            .bind(event_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn insert_backup(&self, backup: &Backup) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO backups (event_id, remote_path, backup_time, size_bytes)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&backup.event_id)
        .bind(&backup.remote_path)
        .bind(backup.backup_time.timestamp())
        .bind(backup.size_bytes as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_events_not_backed_up(&self) -> Result<Vec<Event>> {
        let rows = sqlx::query("SELECT * FROM events WHERE backed_up = FALSE")
            .fetch_all(&self.pool)
            .await?;

        let mut events = Vec::new();
        for row in rows {
            let event = Event {
                id: row.get("id"),
                event_type: row.get("event_type"),
                camera_id: row.get("camera_id"),
                start_time: DateTime::from_timestamp(row.get::<i64, _>("start_time"), 0).unwrap(),
                end_time: DateTime::from_timestamp(row.get::<i64, _>("end_time"), 0).unwrap(),
                backed_up: row.get("backed_up"),
            };
            events.push(event);
        }

        Ok(events)
    }

    pub async fn get_events_by_camera(&self, camera_id: &str) -> Result<Vec<Event>> {
        let rows = sqlx::query("SELECT * FROM events WHERE camera_id = ?")
            .bind(camera_id)
            .fetch_all(&self.pool)
            .await?;

        let mut events = Vec::new();
        for row in rows {
            let event = Event {
                id: row.get("id"),
                event_type: row.get("event_type"),
                camera_id: row.get("camera_id"),
                start_time: DateTime::from_timestamp(row.get::<i64, _>("start_time"), 0).unwrap(),
                end_time: DateTime::from_timestamp(row.get::<i64, _>("end_time"), 0).unwrap(),
                backed_up: row.get("backed_up"),
            };
            events.push(event);
        }

        Ok(events)
    }

    pub async fn cleanup_old_events(&self, retention_days: u32) -> Result<()> {
        let cutoff_time = Utc::now() - chrono::Duration::days(retention_days as i64);

        sqlx::query("DELETE FROM events WHERE start_time < ?")
            .bind(cutoff_time.timestamp())
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
