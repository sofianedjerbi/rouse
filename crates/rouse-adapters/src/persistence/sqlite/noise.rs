use async_trait::async_trait;

use rouse_core::alert::noise::NoiseScore;
use rouse_ports::error::PortError;
use rouse_ports::outbound::NoiseRepository;

use super::SqliteDb;

#[async_trait]
impl NoiseRepository for SqliteDb {
    async fn get_or_create(&self, fingerprint: &str) -> Result<NoiseScore, PortError> {
        let row: Option<(String, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT fingerprint, total_fires, dismissed_count, acted_on_count, avg_time_to_ack_secs
             FROM noise_scores WHERE fingerprint = ?",
        )
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        match row {
            Some((fp, fires, dismissed, acted, avg_ack)) => {
                let data = serde_json::json!({
                    "fingerprint": fp,
                    "total_fires": fires,
                    "dismissed_count": dismissed,
                    "acted_on_count": acted,
                    "avg_time_to_ack_secs": avg_ack,
                });
                let score: NoiseScore = serde_json::from_value(data)
                    .map_err(|e| PortError::Persistence(e.to_string()))?;
                Ok(score)
            }
            None => Ok(NoiseScore::new(fingerprint.to_string())),
        }
    }

    async fn save(&self, score: &NoiseScore) -> Result<(), PortError> {
        sqlx::query(
            "INSERT INTO noise_scores (fingerprint, total_fires, dismissed_count, acted_on_count, avg_time_to_ack_secs)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(fingerprint) DO UPDATE SET
                total_fires = excluded.total_fires,
                dismissed_count = excluded.dismissed_count,
                acted_on_count = excluded.acted_on_count,
                avg_time_to_ack_secs = excluded.avg_time_to_ack_secs",
        )
        .bind(score.fingerprint())
        .bind(score.total_fires() as i64)
        .bind(score.dismissed_count() as i64)
        .bind(score.acted_on_count() as i64)
        .bind(score.avg_time_to_ack().num_seconds())
        .execute(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        Ok(())
    }

    async fn get_noisiest(&self, min_fires: u64) -> Result<Vec<NoiseScore>, PortError> {
        let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT fingerprint, total_fires, dismissed_count, acted_on_count, avg_time_to_ack_secs
             FROM noise_scores
             WHERE total_fires >= ?
             ORDER BY CAST(dismissed_count AS REAL) / CAST(total_fires AS REAL) DESC",
        )
        .bind(min_fires as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| PortError::Persistence(e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for (fp, fires, dismissed, acted, avg_ack) in rows {
            let data = serde_json::json!({
                "fingerprint": fp,
                "total_fires": fires,
                "dismissed_count": dismissed,
                "acted_on_count": acted,
                "avg_time_to_ack_secs": avg_ack,
            });
            let score: NoiseScore =
                serde_json::from_value(data).map_err(|e| PortError::Persistence(e.to_string()))?;
            result.push(score);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> SqliteDb {
        SqliteDb::new("sqlite::memory:").await.unwrap()
    }

    #[tokio::test]
    async fn get_or_create_returns_default() {
        let db = db().await;
        let score = db.get_or_create("fp1").await.unwrap();
        assert_eq!(score.fingerprint(), "fp1");
        assert_eq!(score.total_fires(), 0);
    }

    #[tokio::test]
    async fn save_and_get_or_create_round_trips() {
        let db = db().await;
        let mut score = NoiseScore::new("fp1".into());
        score.record_fire();
        score.record_fire();
        score.record_dismiss();

        db.save(&score).await.unwrap();

        let loaded = db.get_or_create("fp1").await.unwrap();
        assert_eq!(loaded.total_fires(), 2);
        assert_eq!(loaded.dismissed_count(), 1);
    }

    #[tokio::test]
    async fn get_noisiest_filters_and_sorts() {
        let db = db().await;

        // fp1: 10 fires, 8 dismissed (score 0.8)
        let mut s1 = NoiseScore::new("fp1".into());
        for _ in 0..10 {
            s1.record_fire();
        }
        for _ in 0..8 {
            s1.record_dismiss();
        }
        db.save(&s1).await.unwrap();

        // fp2: 5 fires, 5 dismissed (score 1.0)
        let mut s2 = NoiseScore::new("fp2".into());
        for _ in 0..5 {
            s2.record_fire();
        }
        for _ in 0..5 {
            s2.record_dismiss();
        }
        db.save(&s2).await.unwrap();

        // fp3: 2 fires (below min_fires threshold)
        let mut s3 = NoiseScore::new("fp3".into());
        s3.record_fire();
        s3.record_fire();
        db.save(&s3).await.unwrap();

        let noisiest = db.get_noisiest(3).await.unwrap();
        assert_eq!(noisiest.len(), 2);
        assert_eq!(noisiest[0].fingerprint(), "fp2"); // score 1.0 first
        assert_eq!(noisiest[1].fingerprint(), "fp1"); // score 0.8 second
    }
}
