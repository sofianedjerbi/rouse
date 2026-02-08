mod alert;
mod escalation;
mod escalation_queue;
mod event;
mod group;
mod noise;
mod notification_queue;
mod schedule;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

use rouse_ports::error::PortError;

#[derive(Clone)]
pub struct SqliteDb {
    pool: SqlitePool,
}

impl SqliteDb {
    pub async fn new(url: &str) -> Result<Self, PortError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await
            .map_err(|e| PortError::Connection(e.to_string()))?;

        let db = Self { pool };
        db.init_schema().await?;
        Ok(db)
    }

    async fn init_schema(&self) -> Result<(), PortError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS alerts (
                id TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL,
                status TEXT NOT NULL,
                severity TEXT NOT NULL,
                source TEXT NOT NULL,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_alerts_fingerprint ON alerts(fingerprint)")
            .execute(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schedules (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS escalation_policies (
                id TEXT PRIMARY KEY,
                data TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS notifications (
                id TEXT PRIMARY KEY,
                alert_id TEXT NOT NULL,
                channel TEXT NOT NULL,
                target TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                next_attempt_at TEXT NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_notifications_pending
             ON notifications(status, next_attempt_at)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS escalation_steps (
                id TEXT PRIMARY KEY,
                alert_id TEXT NOT NULL,
                policy_id TEXT NOT NULL,
                step_order INTEGER NOT NULL,
                fires_at TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending'
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_escalation_steps_pending
             ON escalation_steps(status, fires_at)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                data TEXT NOT NULL,
                occurred_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS alert_groups (
                id TEXT PRIMARY KEY,
                grouping_key TEXT NOT NULL,
                data TEXT NOT NULL,
                last_added_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_alert_groups_key ON alert_groups(grouping_key)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS noise_scores (
                fingerprint TEXT PRIMARY KEY,
                total_fires INTEGER NOT NULL DEFAULT 0,
                dismissed_count INTEGER NOT NULL DEFAULT 0,
                acted_on_count INTEGER NOT NULL DEFAULT 0,
                avg_time_to_ack_secs INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}
