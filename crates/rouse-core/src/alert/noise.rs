use chrono::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseScore {
    fingerprint: String,
    total_fires: u64,
    dismissed_count: u64,
    acted_on_count: u64,
    avg_time_to_ack_secs: i64,
}

impl NoiseScore {
    pub fn new(fingerprint: String) -> Self {
        Self {
            fingerprint,
            total_fires: 0,
            dismissed_count: 0,
            acted_on_count: 0,
            avg_time_to_ack_secs: 0,
        }
    }

    pub fn record_fire(&mut self) {
        self.total_fires += 1;
    }

    pub fn record_dismiss(&mut self) {
        self.dismissed_count += 1;
    }

    pub fn record_action(&mut self) {
        self.acted_on_count += 1;
    }

    pub fn update_avg_ack_time(&mut self, ack_duration: Duration) {
        let count = self.dismissed_count + self.acted_on_count;
        if count == 0 {
            self.avg_time_to_ack_secs = ack_duration.num_seconds();
        } else {
            // Running average
            let prev_total = self.avg_time_to_ack_secs * (count as i64 - 1);
            self.avg_time_to_ack_secs = (prev_total + ack_duration.num_seconds()) / count as i64;
        }
    }

    /// Score from 0.0 (useful) to 1.0 (pure noise).
    pub fn score(&self) -> f64 {
        if self.total_fires == 0 {
            return 0.0;
        }
        self.dismissed_count as f64 / self.total_fires as f64
    }

    pub fn is_noise(&self) -> bool {
        self.score() > 0.8
    }

    pub fn suggest_suppression(&self) -> bool {
        self.score() > 0.95
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn total_fires(&self) -> u64 {
        self.total_fires
    }

    pub fn dismissed_count(&self) -> u64 {
        self.dismissed_count
    }

    pub fn acted_on_count(&self) -> u64 {
        self.acted_on_count
    }

    pub fn avg_time_to_ack(&self) -> Duration {
        Duration::seconds(self.avg_time_to_ack_secs)
    }
}

/// Classify an ack/resolve pair as dismiss or action.
pub fn classify_response(time_to_ack: Duration, time_to_resolve: Option<Duration>) -> bool {
    // Dismissed if:
    // - Acked within 5 seconds (reflexive)
    // - OR resolved within 60 seconds of ack (nothing was actually done)
    if time_to_ack < Duration::seconds(5) {
        return true; // dismissed
    }
    if let Some(resolve_time) = time_to_resolve {
        if resolve_time < Duration::seconds(60) {
            return true; // dismissed
        }
    }
    false // acted on
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_zero_when_no_fires() {
        let score = NoiseScore::new("fp1".into());
        assert_eq!(score.score(), 0.0);
    }

    #[test]
    fn score_calculation_correct() {
        let mut score = NoiseScore::new("fp1".into());
        for _ in 0..10 {
            score.record_fire();
        }
        for _ in 0..8 {
            score.record_dismiss();
        }
        for _ in 0..2 {
            score.record_action();
        }
        assert!((score.score() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn high_score_is_noise() {
        let mut score = NoiseScore::new("fp1".into());
        for _ in 0..10 {
            score.record_fire();
            score.record_dismiss();
        }
        assert!(score.is_noise());
    }

    #[test]
    fn low_score_is_not_noise() {
        let mut score = NoiseScore::new("fp1".into());
        for _ in 0..10 {
            score.record_fire();
            score.record_action();
        }
        assert!(!score.is_noise());
        assert_eq!(score.score(), 0.0);
    }

    #[test]
    fn quick_ack_is_dismiss() {
        let ack_time = Duration::seconds(2);
        assert!(classify_response(ack_time, None));
    }

    #[test]
    fn slow_ack_is_action() {
        let ack_time = Duration::minutes(5);
        assert!(!classify_response(ack_time, None));
    }

    #[test]
    fn quick_resolve_after_slow_ack_is_dismiss() {
        let ack_time = Duration::seconds(30);
        let resolve_time = Duration::seconds(45); // resolved 45s after ack
        assert!(classify_response(ack_time, Some(resolve_time)));
    }

    #[test]
    fn suggest_suppression_above_threshold() {
        let mut score = NoiseScore::new("fp1".into());
        for _ in 0..100 {
            score.record_fire();
        }
        for _ in 0..96 {
            score.record_dismiss();
        }
        assert!(score.suggest_suppression());
    }
}
