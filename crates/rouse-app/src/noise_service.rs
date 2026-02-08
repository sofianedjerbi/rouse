use chrono::{DateTime, Utc};

use rouse_core::alert::noise::{classify_response, NoiseScore};
use rouse_ports::outbound::NoiseRepository;

use crate::error::AppError;

pub struct NoiseService<NR>
where
    NR: NoiseRepository,
{
    noise_repo: NR,
}

impl<NR> NoiseService<NR>
where
    NR: NoiseRepository,
{
    pub fn new(noise_repo: NR) -> Self {
        Self { noise_repo }
    }

    /// Record a new alert fire for noise tracking.
    pub async fn record_fire(&self, fingerprint: &str) -> Result<(), AppError> {
        let mut score = self.noise_repo.get_or_create(fingerprint).await?;
        score.record_fire();
        self.noise_repo.save(&score).await?;
        Ok(())
    }

    /// Classify and record the operator response to an alert.
    /// Call after an alert is resolved to determine if it was dismissed or acted upon.
    pub async fn record_response(
        &self,
        fingerprint: &str,
        created_at: DateTime<Utc>,
        acknowledged_at: Option<DateTime<Utc>>,
        resolved_at: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut score = self.noise_repo.get_or_create(fingerprint).await?;

        if let Some(acked_at) = acknowledged_at {
            let time_to_ack = acked_at - created_at;
            let time_to_resolve = resolved_at - acked_at;

            let dismissed = classify_response(time_to_ack, Some(time_to_resolve));
            if dismissed {
                score.record_dismiss();
            } else {
                score.record_action();
            }
            score.update_avg_ack_time(time_to_ack);
        } else {
            // Resolved without ack — classify based on total time
            let total_time = resolved_at - created_at;
            let dismissed = classify_response(total_time, None);
            if dismissed {
                score.record_dismiss();
            } else {
                score.record_action();
            }
        }

        self.noise_repo.save(&score).await?;
        Ok(())
    }

    /// Get the noisiest alerts above a minimum fire count.
    pub async fn get_noisy_alerts(&self, min_fires: u64) -> Result<Vec<NoiseScore>, AppError> {
        Ok(self.noise_repo.get_noisiest(min_fires).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rouse_ports::error::PortError;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockNoiseRepo {
        scores: Mutex<Vec<NoiseScore>>,
    }

    #[async_trait]
    impl NoiseRepository for MockNoiseRepo {
        async fn get_or_create(&self, fingerprint: &str) -> Result<NoiseScore, PortError> {
            let scores = self.scores.lock().unwrap();
            Ok(scores
                .iter()
                .find(|s| s.fingerprint() == fingerprint)
                .cloned()
                .unwrap_or_else(|| NoiseScore::new(fingerprint.to_string())))
        }

        async fn save(&self, score: &NoiseScore) -> Result<(), PortError> {
            let mut scores = self.scores.lock().unwrap();
            if let Some(pos) = scores
                .iter()
                .position(|s| s.fingerprint() == score.fingerprint())
            {
                scores[pos] = score.clone();
            } else {
                scores.push(score.clone());
            }
            Ok(())
        }

        async fn get_noisiest(&self, min_fires: u64) -> Result<Vec<NoiseScore>, PortError> {
            let scores = self.scores.lock().unwrap();
            let mut result: Vec<_> = scores
                .iter()
                .filter(|s| s.total_fires() >= min_fires)
                .cloned()
                .collect();
            result.sort_by(|a, b| b.score().partial_cmp(&a.score()).unwrap());
            Ok(result)
        }
    }

    fn ts(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn make_service() -> NoiseService<MockNoiseRepo> {
        NoiseService::new(MockNoiseRepo::default())
    }

    #[tokio::test]
    async fn record_fire_increments_total() {
        let svc = make_service();
        svc.record_fire("fp1").await.unwrap();
        svc.record_fire("fp1").await.unwrap();

        let scores = svc.noise_repo.scores.lock().unwrap();
        let score = scores.iter().find(|s| s.fingerprint() == "fp1").unwrap();
        assert_eq!(score.total_fires(), 2);
    }

    #[tokio::test]
    async fn quick_ack_and_resolve_records_dismiss() {
        let svc = make_service();
        svc.record_fire("fp1").await.unwrap();

        let created = ts("2025-01-15T10:00:00Z");
        let acked = ts("2025-01-15T10:00:02Z"); // 2s — reflexive ack
        let resolved = ts("2025-01-15T10:00:30Z");

        svc.record_response("fp1", created, Some(acked), resolved)
            .await
            .unwrap();

        let scores = svc.noise_repo.scores.lock().unwrap();
        let score = scores.iter().find(|s| s.fingerprint() == "fp1").unwrap();
        assert_eq!(score.dismissed_count(), 1);
        assert_eq!(score.acted_on_count(), 0);
    }

    #[tokio::test]
    async fn slow_ack_and_long_resolve_records_action() {
        let svc = make_service();
        svc.record_fire("fp1").await.unwrap();

        let created = ts("2025-01-15T10:00:00Z");
        let acked = ts("2025-01-15T10:05:00Z"); // 5min — deliberate
        let resolved = ts("2025-01-15T10:30:00Z"); // 25min after ack

        svc.record_response("fp1", created, Some(acked), resolved)
            .await
            .unwrap();

        let scores = svc.noise_repo.scores.lock().unwrap();
        let score = scores.iter().find(|s| s.fingerprint() == "fp1").unwrap();
        assert_eq!(score.dismissed_count(), 0);
        assert_eq!(score.acted_on_count(), 1);
    }

    #[tokio::test]
    async fn resolve_without_ack_quick_is_dismiss() {
        let svc = make_service();
        svc.record_fire("fp1").await.unwrap();

        let created = ts("2025-01-15T10:00:00Z");
        let resolved = ts("2025-01-15T10:00:03Z"); // 3s — auto-resolved

        svc.record_response("fp1", created, None, resolved)
            .await
            .unwrap();

        let scores = svc.noise_repo.scores.lock().unwrap();
        let score = scores.iter().find(|s| s.fingerprint() == "fp1").unwrap();
        assert_eq!(score.dismissed_count(), 1);
    }

    #[tokio::test]
    async fn resolve_without_ack_slow_is_action() {
        let svc = make_service();
        svc.record_fire("fp1").await.unwrap();

        let created = ts("2025-01-15T10:00:00Z");
        let resolved = ts("2025-01-15T10:10:00Z"); // 10min

        svc.record_response("fp1", created, None, resolved)
            .await
            .unwrap();

        let scores = svc.noise_repo.scores.lock().unwrap();
        let score = scores.iter().find(|s| s.fingerprint() == "fp1").unwrap();
        assert_eq!(score.acted_on_count(), 1);
    }

    #[tokio::test]
    async fn noisy_fingerprint_detected_after_repeated_dismissals() {
        let svc = make_service();

        let created = ts("2025-01-15T10:00:00Z");
        let acked = ts("2025-01-15T10:00:01Z"); // 1s
        let resolved = ts("2025-01-15T10:00:10Z");

        for _ in 0..10 {
            svc.record_fire("fp1").await.unwrap();
            svc.record_response("fp1", created, Some(acked), resolved)
                .await
                .unwrap();
        }

        let scores = svc.noise_repo.scores.lock().unwrap();
        let score = scores.iter().find(|s| s.fingerprint() == "fp1").unwrap();
        assert!(score.is_noise());
        assert_eq!(score.total_fires(), 10);
        assert_eq!(score.dismissed_count(), 10);
    }

    #[tokio::test]
    async fn get_noisy_alerts_filters_by_min_fires() {
        let svc = make_service();

        // fp1: 5 fires
        for _ in 0..5 {
            svc.record_fire("fp1").await.unwrap();
        }
        // fp2: 2 fires
        for _ in 0..2 {
            svc.record_fire("fp2").await.unwrap();
        }

        let result = svc.get_noisy_alerts(3).await.unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].fingerprint(), "fp1");
    }
}
