use async_trait::async_trait;
use chrono::{DateTime, Utc};

use rouse_ports::error::PortError;
use rouse_ports::outbound::NotificationQueue;
use rouse_ports::types::{PendingNotification, QueueStatus};

use super::SqliteDb;

fn channel_to_str(ch: &rouse_core::channel::Channel) -> &'static str {
    match ch {
        rouse_core::channel::Channel::Slack => "slack",
        rouse_core::channel::Channel::Discord => "discord",
        rouse_core::channel::Channel::Telegram => "telegram",
        rouse_core::channel::Channel::WhatsApp => "whatsapp",
        rouse_core::channel::Channel::Sms => "sms",
        rouse_core::channel::Channel::Phone => "phone",
        rouse_core::channel::Channel::Email => "email",
        rouse_core::channel::Channel::Webhook => "webhook",
    }
}

fn str_to_channel(s: &str) -> Result<rouse_core::channel::Channel, PortError> {
    match s {
        "slack" => Ok(rouse_core::channel::Channel::Slack),
        "discord" => Ok(rouse_core::channel::Channel::Discord),
        "telegram" => Ok(rouse_core::channel::Channel::Telegram),
        "whatsapp" => Ok(rouse_core::channel::Channel::WhatsApp),
        "sms" => Ok(rouse_core::channel::Channel::Sms),
        "phone" => Ok(rouse_core::channel::Channel::Phone),
        "email" => Ok(rouse_core::channel::Channel::Email),
        "webhook" => Ok(rouse_core::channel::Channel::Webhook),
        other => Err(PortError::Persistence(format!("unknown channel: {other}"))),
    }
}

fn status_to_str(s: &QueueStatus) -> &'static str {
    match s {
        QueueStatus::Pending => "pending",
        QueueStatus::Sent => "sent",
        QueueStatus::Failed => "failed",
        QueueStatus::Dead => "dead",
    }
}

#[async_trait]
impl NotificationQueue for SqliteDb {
    async fn enqueue(&self, notification: PendingNotification) -> Result<(), PortError> {
        let channel = channel_to_str(&notification.channel);
        let status = status_to_str(&notification.status);
        let alert_id = notification.alert_id.to_string();
        let next_attempt = notification.next_attempt_at.to_rfc3339();
        let created_at = notification.created_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO notifications (id, alert_id, channel, target, payload, status, next_attempt_at, retry_count, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&notification.id)
        .bind(&alert_id)
        .bind(channel)
        .bind(&notification.target)
        .bind(&notification.payload)
        .bind(status)
        .bind(&next_attempt)
        .bind(notification.retry_count)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn poll_pending(&self) -> Result<Vec<PendingNotification>, PortError> {
        let now = Utc::now().to_rfc3339();
        let rows: Vec<(String, String, String, String, String, String, String, i32, String)> =
            sqlx::query_as(
                "SELECT id, alert_id, channel, target, payload, status, next_attempt_at, retry_count, created_at
                 FROM notifications
                 WHERE status = 'pending' AND next_attempt_at <= ?
                 ORDER BY next_attempt_at ASC",
            )
            .bind(&now)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for (
            id,
            alert_id,
            channel,
            target,
            payload,
            _status,
            next_attempt,
            retry_count,
            created_at,
        ) in rows
        {
            result.push(PendingNotification {
                id,
                alert_id: rouse_core::ids::AlertId::parse(&alert_id)
                    .map_err(|e| PortError::Persistence(e.to_string()))?,
                channel: str_to_channel(&channel)?,
                target,
                payload,
                status: QueueStatus::Pending,
                next_attempt_at: DateTime::parse_from_rfc3339(&next_attempt)
                    .map_err(|e| PortError::Persistence(e.to_string()))?
                    .with_timezone(&Utc),
                retry_count: retry_count as u32,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| PortError::Persistence(e.to_string()))?
                    .with_timezone(&Utc),
            });
        }
        Ok(result)
    }

    async fn mark_sent(&self, id: &str) -> Result<(), PortError> {
        sqlx::query("UPDATE notifications SET status = 'sent' WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        id: &str,
        error: &str,
        next_attempt: DateTime<Utc>,
    ) -> Result<(), PortError> {
        let next = next_attempt.to_rfc3339();
        sqlx::query(
            "UPDATE notifications SET status = 'failed', next_attempt_at = ?, retry_count = retry_count + 1 WHERE id = ?",
        )
        .bind(&next)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        // Store error in a separate column would be better, but schema doesn't have it yet.
        // For now, we log it via tracing
        tracing::warn!(notification_id = id, error = error, "notification failed");

        Ok(())
    }

    async fn mark_dead(&self, id: &str) -> Result<(), PortError> {
        sqlx::query("UPDATE notifications SET status = 'dead' WHERE id = ?")
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
    use rouse_core::channel::Channel;
    use rouse_core::ids::AlertId;

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    fn make_notification(alert_id: &AlertId) -> PendingNotification {
        PendingNotification {
            id: uuid::Uuid::new_v4().to_string(),
            alert_id: alert_id.clone(),
            channel: Channel::Slack,
            target: "#oncall".into(),
            payload: r#"{"text":"alert fired"}"#.into(),
            status: QueueStatus::Pending,
            next_attempt_at: Utc::now() - chrono::Duration::seconds(10),
            retry_count: 0,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn enqueue_and_poll_pending() {
        let db = db().await;
        let alert_id = AlertId::new();
        let notif = make_notification(&alert_id);
        let notif_id = notif.id.clone();

        db.enqueue(notif).await.unwrap();

        let pending = db.poll_pending().await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, notif_id);
        assert_eq!(pending[0].channel, Channel::Slack);
    }

    #[tokio::test]
    async fn mark_sent_removes_from_pending() {
        let db = db().await;
        let alert_id = AlertId::new();
        let notif = make_notification(&alert_id);
        let notif_id = notif.id.clone();

        db.enqueue(notif).await.unwrap();
        db.mark_sent(&notif_id).await.unwrap();

        let pending = db.poll_pending().await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn mark_dead_removes_from_pending() {
        let db = db().await;
        let alert_id = AlertId::new();
        let notif = make_notification(&alert_id);
        let notif_id = notif.id.clone();

        db.enqueue(notif).await.unwrap();
        db.mark_dead(&notif_id).await.unwrap();

        let pending = db.poll_pending().await.unwrap();
        assert!(pending.is_empty());
    }
}
