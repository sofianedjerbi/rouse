use async_trait::async_trait;
use chrono::{DateTime, Utc};

use rouse_ports::error::PortError;
use rouse_ports::outbound::EscalationQueue;
use rouse_ports::types::{PendingEscalation, QueueStatus};

use super::SqliteDb;

#[async_trait]
impl EscalationQueue for SqliteDb {
    async fn enqueue_step(&self, step: PendingEscalation) -> Result<(), PortError> {
        let alert_id = step.alert_id.to_string();
        let policy_id = step.policy_id.to_string();
        let fires_at = step.fires_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO escalation_steps (id, alert_id, policy_id, step_order, fires_at, status)
             VALUES (?, ?, ?, ?, ?, 'pending')",
        )
        .bind(&step.id)
        .bind(&alert_id)
        .bind(&policy_id)
        .bind(step.step_order)
        .bind(&fires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn poll_due(&self) -> Result<Vec<PendingEscalation>, PortError> {
        let now = Utc::now().to_rfc3339();
        let rows: Vec<(String, String, String, i32, String, String)> = sqlx::query_as(
            "SELECT id, alert_id, policy_id, step_order, fires_at, status
             FROM escalation_steps
             WHERE status = 'pending' AND fires_at <= ?
             ORDER BY fires_at ASC",
        )
        .bind(&now)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for (id, alert_id, policy_id, step_order, fires_at, _status) in rows {
            result.push(PendingEscalation {
                id,
                alert_id: rouse_core::ids::AlertId::parse(&alert_id)
                    .map_err(|e| PortError::Persistence(e.to_string()))?,
                policy_id: rouse_core::ids::PolicyId::parse(&policy_id)
                    .map_err(|e| PortError::Persistence(e.to_string()))?,
                step_order: step_order as u32,
                fires_at: DateTime::parse_from_rfc3339(&fires_at)
                    .map_err(|e| PortError::Persistence(e.to_string()))?
                    .with_timezone(&Utc),
                status: QueueStatus::Pending,
            });
        }
        Ok(result)
    }

    async fn cancel_for_alert(&self, alert_id: &str) -> Result<(), PortError> {
        sqlx::query(
            "UPDATE escalation_steps SET status = 'cancelled' WHERE alert_id = ? AND status = 'pending'",
        )
        .bind(alert_id)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn mark_fired(&self, id: &str) -> Result<(), PortError> {
        sqlx::query("UPDATE escalation_steps SET status = 'fired' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rouse_core::ids::{AlertId, PolicyId};

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    fn make_step(alert_id: &AlertId) -> PendingEscalation {
        PendingEscalation {
            id: uuid::Uuid::new_v4().to_string(),
            alert_id: alert_id.clone(),
            policy_id: PolicyId::new(),
            step_order: 0,
            fires_at: chrono::Utc::now() - chrono::Duration::seconds(10),
            status: QueueStatus::Pending,
        }
    }

    #[tokio::test]
    async fn enqueue_and_poll_due() {
        let db = db().await;
        let alert_id = AlertId::new();
        let step = make_step(&alert_id);
        let step_id = step.id.clone();

        db.enqueue_step(step).await.unwrap();

        let due = db.poll_due().await.unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].id, step_id);
    }

    #[tokio::test]
    async fn cancel_for_alert_removes_pending() {
        let db = db().await;
        let alert_id = AlertId::new();
        db.enqueue_step(make_step(&alert_id)).await.unwrap();

        db.cancel_for_alert(&alert_id.to_string()).await.unwrap();

        let due = db.poll_due().await.unwrap();
        assert!(due.is_empty());
    }

    #[tokio::test]
    async fn mark_fired_removes_from_pending() {
        let db = db().await;
        let alert_id = AlertId::new();
        let step = make_step(&alert_id);
        let step_id = step.id.clone();

        db.enqueue_step(step).await.unwrap();
        db.mark_fired(&step_id).await.unwrap();

        let due = db.poll_due().await.unwrap();
        assert!(due.is_empty());
    }
}
