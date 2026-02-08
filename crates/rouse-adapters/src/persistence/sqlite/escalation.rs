use async_trait::async_trait;

use rouse_core::escalation::EscalationPolicy;
use rouse_ports::error::PortError;
use rouse_ports::outbound::EscalationRepository;

use super::SqliteDb;

#[async_trait]
impl EscalationRepository for SqliteDb {
    async fn save(&self, policy: &EscalationPolicy) -> Result<(), PortError> {
        let id = policy.id().to_string();
        let data =
            serde_json::to_string(policy).map_err(|e| PortError::Persistence(e.to_string()))?;

        sqlx::query(
            "INSERT INTO escalation_policies (id, data) VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
        )
        .bind(&id)
        .bind(&data)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<EscalationPolicy>, PortError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT data FROM escalation_policies WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PortError::Persistence(e.to_string()))?;

        match row {
            Some((data,)) => {
                let policy: EscalationPolicy = serde_json::from_str(&data)
                    .map_err(|e| PortError::Persistence(e.to_string()))?;
                Ok(Some(policy))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rouse_core::channel::Channel;
    use rouse_core::escalation::{EscalationStep, EscalationTarget};
    use rouse_core::ids::UserId;

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    fn make_policy() -> EscalationPolicy {
        EscalationPolicy::new(
            "critical".into(),
            vec![EscalationStep::new(
                0,
                0,
                vec![EscalationTarget::User(UserId::new())],
                vec![Channel::Slack],
            )],
            1,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let db = db().await;
        let policy = make_policy();
        let id = policy.id().to_string();

        db.save(&policy).await.unwrap();

        let found = db.find_by_id(&id).await.unwrap().unwrap();
        assert_eq!(found.name(), "critical");
        assert_eq!(found.repeat_count(), 1);
    }

    #[tokio::test]
    async fn find_by_id_returns_none() {
        let db = db().await;
        let found = db
            .find_by_id("00000000-0000-0000-0000-000000000000")
            .await
            .unwrap();
        assert!(found.is_none());
    }
}
