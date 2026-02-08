use async_trait::async_trait;

use rouse_core::alert::Alert;
use rouse_ports::error::PortError;
use rouse_ports::outbound::AlertRepository;
use rouse_ports::types::AlertFilter;

use super::SqliteDb;

#[async_trait]
impl AlertRepository for SqliteDb {
    async fn save(&self, alert: &Alert) -> Result<(), PortError> {
        let id = alert.id().to_string();
        let fingerprint = alert.fingerprint().as_str().to_string();
        let status = format!("{:?}", alert.status());
        let severity = format!("{:?}", alert.severity());
        let source = alert.source().as_str().to_string();
        let data =
            serde_json::to_string(alert).map_err(|e| PortError::Persistence(e.to_string()))?;
        let created_at = alert.created_at().to_rfc3339();

        sqlx::query(
            "INSERT INTO alerts (id, fingerprint, status, severity, source, data, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                fingerprint = excluded.fingerprint,
                status = excluded.status,
                severity = excluded.severity,
                source = excluded.source,
                data = excluded.data",
        )
        .bind(&id)
        .bind(&fingerprint)
        .bind(&status)
        .bind(&severity)
        .bind(&source)
        .bind(&data)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Alert>, PortError> {
        let row: Option<(String,)> = sqlx::query_as("SELECT data FROM alerts WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;

        match row {
            Some((data,)) => {
                let alert: Alert = serde_json::from_str(&data)
                    .map_err(|e| PortError::Persistence(e.to_string()))?;
                Ok(Some(alert))
            }
            None => Ok(None),
        }
    }

    async fn find_by_fingerprint(&self, fp: &str) -> Result<Option<Alert>, PortError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT data FROM alerts WHERE fingerprint = ? LIMIT 1")
                .bind(fp)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PortError::Persistence(e.to_string()))?;

        match row {
            Some((data,)) => {
                let alert: Alert = serde_json::from_str(&data)
                    .map_err(|e| PortError::Persistence(e.to_string()))?;
                Ok(Some(alert))
            }
            None => Ok(None),
        }
    }

    async fn find_by_filter(&self, filter: &AlertFilter) -> Result<Vec<Alert>, PortError> {
        let mut sql = String::from("SELECT data FROM alerts WHERE 1=1");
        let mut binds: Vec<String> = Vec::new();

        if let Some(status) = &filter.status {
            sql.push_str(" AND status = ?");
            binds.push(format!("{status:?}"));
        }
        if let Some(severity) = &filter.severity {
            sql.push_str(" AND severity = ?");
            binds.push(format!("{severity:?}"));
        }
        if let Some(source) = &filter.source {
            sql.push_str(" AND source = ?");
            binds.push(source.clone());
        }
        if let Some(search) = &filter.search {
            sql.push_str(" AND data LIKE ?");
            binds.push(format!("%{search}%"));
        }

        sql.push_str(" ORDER BY created_at DESC");

        let per_page = if filter.per_page == 0 {
            50
        } else {
            filter.per_page
        };
        let offset = filter.page.saturating_sub(1) * per_page;
        sql.push_str(&format!(" LIMIT {per_page} OFFSET {offset}"));

        let mut query = sqlx::query_as::<_, (String,)>(&sql);
        for b in &binds {
            query = query.bind(b);
        }

        let rows = query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PortError::Persistence(e.to_string()))?;

        let mut alerts = Vec::with_capacity(rows.len());
        for (data,) in rows {
            let alert: Alert =
                serde_json::from_str(&data).map_err(|e| PortError::Persistence(e.to_string()))?;
            alerts.push(alert);
        }
        Ok(alerts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rouse_core::alert::{Severity, Source, Status};
    use std::collections::BTreeMap;

    fn ts(s: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    fn make_alert(service: &str) -> Alert {
        let labels = BTreeMap::from([("service".into(), service.into())]);
        let (alert, _) = Alert::new(
            "ext-1".into(),
            Source::new("alertmanager"),
            Severity::Critical,
            labels,
            "High CPU".into(),
            ts("2025-01-15T10:00:00Z"),
        );
        alert
    }

    #[tokio::test]
    async fn save_and_find_by_id() {
        let db = db().await;
        let alert = make_alert("api");
        let id = alert.id().to_string();

        db.save(&alert).await.unwrap();

        let found = db.find_by_id(&id).await.unwrap().unwrap();
        assert_eq!(found.id(), alert.id());
        assert_eq!(found.status(), Status::Firing);
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

    #[tokio::test]
    async fn save_and_find_by_fingerprint() {
        let db = db().await;
        let alert = make_alert("payments");

        db.save(&alert).await.unwrap();

        let found = db
            .find_by_fingerprint(alert.fingerprint().as_str())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id(), alert.id());
    }

    #[tokio::test]
    async fn save_updates_existing() {
        let db = db().await;
        let (mut alert, _) = Alert::new(
            "ext-1".into(),
            Source::new("am"),
            Severity::Warning,
            BTreeMap::new(),
            "test".into(),
            ts("2025-01-15T10:00:00Z"),
        );
        let id = alert.id().to_string();

        db.save(&alert).await.unwrap();

        let user_id = rouse_core::ids::UserId::new();
        alert
            .acknowledge(user_id, ts("2025-01-15T10:01:00Z"))
            .unwrap();
        db.save(&alert).await.unwrap();

        let found = db.find_by_id(&id).await.unwrap().unwrap();
        assert_eq!(found.status(), Status::Acknowledged);
    }

    #[tokio::test]
    async fn find_by_filter_status() {
        let db = db().await;
        let alert = make_alert("api");
        db.save(&alert).await.unwrap();

        let filter = AlertFilter {
            status: Some(Status::Firing),
            page: 1,
            per_page: 50,
            ..Default::default()
        };
        let results = db.find_by_filter(&filter).await.unwrap();
        assert_eq!(results.len(), 1);

        let filter = AlertFilter {
            status: Some(Status::Resolved),
            page: 1,
            per_page: 50,
            ..Default::default()
        };
        let results = db.find_by_filter(&filter).await.unwrap();
        assert!(results.is_empty());
    }
}
