use async_trait::async_trait;

use rouse_core::alert::group::AlertGroup;
use rouse_ports::error::PortError;
use rouse_ports::outbound::AlertGroupRepository;

use super::SqliteDb;

#[async_trait]
impl AlertGroupRepository for SqliteDb {
    async fn save(&self, group: &AlertGroup) -> Result<(), PortError> {
        let id = group.id().to_string();
        let data =
            serde_json::to_string(group).map_err(|e| PortError::Persistence(e.to_string()))?;
        let last_added_at = group.last_added_at().to_rfc3339();

        sqlx::query(
            "INSERT INTO alert_groups (id, grouping_key, data, last_added_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                last_added_at = excluded.last_added_at",
        )
        .bind(&id)
        .bind(group.grouping_key())
        .bind(&data)
        .bind(&last_added_at)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn find_active_by_key(&self, key: &str) -> Result<Option<AlertGroup>, PortError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT data FROM alert_groups WHERE grouping_key = ? LIMIT 1")
                .bind(key)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PortError::Persistence(e.to_string()))?;

        match row {
            Some((data,)) => {
                let group: AlertGroup = serde_json::from_str(&data)
                    .map_err(|e| PortError::Persistence(e.to_string()))?;
                Ok(Some(group))
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use rouse_core::ids::AlertId;

    fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn save_and_find_active_by_key() {
        let db = db().await;
        let group = AlertGroup::new(
            AlertId::new(),
            "am:api".into(),
            Duration::seconds(30),
            ts("2025-01-15T10:00:00Z"),
        );

        db.save(&group).await.unwrap();

        let found = db.find_active_by_key("am:api").await.unwrap().unwrap();
        assert_eq!(found.id(), group.id());
        assert_eq!(found.member_count(), 1);
    }

    #[tokio::test]
    async fn find_active_by_key_returns_none() {
        let db = db().await;
        let found = db.find_active_by_key("nonexistent").await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn save_updates_existing_group() {
        let db = db().await;
        let mut group = AlertGroup::new(
            AlertId::new(),
            "am:api".into(),
            Duration::seconds(30),
            ts("2025-01-15T10:00:00Z"),
        );
        db.save(&group).await.unwrap();

        group.add_member(AlertId::new(), ts("2025-01-15T10:00:05Z"));
        db.save(&group).await.unwrap();

        let found = db.find_active_by_key("am:api").await.unwrap().unwrap();
        assert_eq!(found.member_count(), 2);
    }
}
