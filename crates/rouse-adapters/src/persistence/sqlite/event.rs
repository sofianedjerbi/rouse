use async_trait::async_trait;

use rouse_core::events::DomainEvent;
use rouse_ports::error::PortError;
use rouse_ports::outbound::EventPublisher;

use super::SqliteDb;

#[async_trait]
impl EventPublisher for SqliteDb {
    async fn publish(&self, events: Vec<DomainEvent>) -> Result<(), PortError> {
        for event in &events {
            let event_type = event.event_type();
            let data =
                serde_json::to_string(event).map_err(|e| PortError::Persistence(e.to_string()))?;
            let occurred_at = event.occurred_at().to_rfc3339();

            sqlx::query("INSERT INTO events (event_type, data, occurred_at) VALUES (?, ?, ?)")
                .bind(event_type)
                .bind(&data)
                .bind(&occurred_at)
                .execute(&self.pool)
                .await
                .map_err(|e| PortError::Persistence(e.to_string()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rouse_core::alert::Severity;
    use rouse_core::events::AlertReceived;
    use rouse_core::ids::AlertId;

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    #[tokio::test]
    async fn publish_stores_events() {
        let db = db().await;

        let events = vec![
            DomainEvent::AlertReceived(AlertReceived {
                alert_id: AlertId::new(),
                source: "alertmanager".into(),
                severity: Severity::Critical,
                occurred_at: ts("2025-01-15T10:00:00Z"),
            }),
            DomainEvent::AlertReceived(AlertReceived {
                alert_id: AlertId::new(),
                source: "datadog".into(),
                severity: Severity::Warning,
                occurred_at: ts("2025-01-15T10:01:00Z"),
            }),
        ];

        db.publish(events).await.unwrap();

        // Verify events were stored
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM events")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(count.0, 2);
    }
}
