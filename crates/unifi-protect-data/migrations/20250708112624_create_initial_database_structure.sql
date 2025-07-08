-- Add migration script here
CREATE TABLE IF NOT EXISTS events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    camera_id TEXT NOT NULL,
    start_time INTEGER NOT NULL,
    end_time INTEGER,
    backed_up BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE IF NOT EXISTS backups (
    event_id TEXT NOT NULL,
    remote_path TEXT NOT NULL,
    backup_time INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    PRIMARY KEY (event_id, remote_path),
    FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
);

PRAGMA foreign_keys = ON;