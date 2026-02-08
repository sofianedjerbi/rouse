pub mod rotation;
pub mod shift_override;

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::events::{DomainEvent, OnCallChanged};
use crate::ids::{OverrideId, ScheduleId, UserId};

pub use rotation::Rotation;
pub use shift_override::ScheduleOverride;

mod tz_serde {
    use chrono_tz::Tz;
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(tz: &Tz, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(tz.name())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Tz, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<Tz>().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandoffTime {
    pub day: chrono::Weekday,
    pub hour: u32,
    pub minute: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    id: ScheduleId,
    name: String,
    #[serde(with = "tz_serde")]
    timezone: Tz,
    rotation: Rotation,
    participants: Vec<UserId>,
    handoff: HandoffTime,
    overrides: Vec<ScheduleOverride>,
}

impl Schedule {
    pub fn new(
        name: String,
        timezone: Tz,
        rotation: Rotation,
        participants: Vec<UserId>,
        handoff: HandoffTime,
    ) -> Result<Self, DomainError> {
        if participants.is_empty() {
            return Err(DomainError::ScheduleRequiresParticipant);
        }
        Ok(Self {
            id: ScheduleId::new(),
            name,
            timezone,
            rotation,
            participants,
            handoff,
            overrides: vec![],
        })
    }

    pub fn who_is_on_call(&self, at: DateTime<Utc>) -> UserId {
        // Check overrides first (latest added wins)
        for ovr in self.overrides.iter().rev() {
            if ovr.is_active_at(at) {
                return ovr.user_id().clone();
            }
        }

        // Fall back to rotation
        self.rotation_on_call(at)
    }

    fn rotation_on_call(&self, at: DateTime<Utc>) -> UserId {
        let local = at.with_timezone(&self.timezone);
        let rotation_secs = self.rotation.duration().num_seconds();

        // Calculate the epoch for this schedule: first handoff
        // We use a fixed epoch and count rotation periods from there
        let epoch = chrono::NaiveDate::from_ymd_opt(2020, 1, 6) // Monday
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(self.timezone)
            .unwrap();

        let elapsed = local.signed_duration_since(epoch).num_seconds();
        let index = (elapsed / rotation_secs).rem_euclid(self.participants.len() as i64) as usize;

        self.participants[index].clone()
    }

    pub fn add_override(
        &mut self,
        ovr: ScheduleOverride,
        now: DateTime<Utc>,
    ) -> Result<Vec<DomainEvent>, DomainError> {
        if ovr.end() <= ovr.start() {
            return Err(DomainError::InvalidOverridePeriod);
        }
        let new_user = ovr.user_id().clone();
        self.overrides.push(ovr);
        Ok(vec![DomainEvent::OnCallChanged(OnCallChanged {
            schedule_id: self.id.clone(),
            new_user,
            previous_user: None,
            occurred_at: now,
        })])
    }

    pub fn remove_override(
        &mut self,
        override_id: &OverrideId,
        now: DateTime<Utc>,
    ) -> Result<Vec<DomainEvent>, DomainError> {
        let pos = self.overrides.iter().position(|o| o.id() == override_id);
        if let Some(idx) = pos {
            self.overrides.remove(idx);
            let current = self.who_is_on_call(now);
            Ok(vec![DomainEvent::OnCallChanged(OnCallChanged {
                schedule_id: self.id.clone(),
                new_user: current,
                previous_user: None,
                occurred_at: now,
            })])
        } else {
            Ok(vec![])
        }
    }

    pub fn id(&self) -> &ScheduleId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn participants(&self) -> &[UserId] {
        &self.participants
    }

    pub fn handoff(&self) -> &HandoffTime {
        &self.handoff
    }

    pub fn timezone(&self) -> &Tz {
        &self.timezone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zurich() -> Tz {
        "Europe/Zurich".parse().unwrap()
    }

    fn handoff_monday_9() -> HandoffTime {
        HandoffTime {
            day: chrono::Weekday::Mon,
            hour: 9,
            minute: 0,
        }
    }

    fn ts(s: &str) -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339(s)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn make_users(n: usize) -> Vec<UserId> {
        (0..n).map(|_| UserId::new()).collect()
    }

    #[test]
    fn schedule_requires_at_least_one_participant() {
        let result = Schedule::new(
            "empty".into(),
            zurich(),
            Rotation::Weekly,
            vec![],
            handoff_monday_9(),
        );
        assert!(matches!(
            result,
            Err(DomainError::ScheduleRequiresParticipant)
        ));
    }

    #[test]
    fn single_participant_always_on_call() {
        let users = make_users(1);
        let sched = Schedule::new(
            "solo".into(),
            zurich(),
            Rotation::Weekly,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        // Check multiple different times
        assert_eq!(sched.who_is_on_call(ts("2025-01-15T10:00:00Z")), users[0]);
        assert_eq!(sched.who_is_on_call(ts("2025-06-20T03:00:00Z")), users[0]);
    }

    #[test]
    fn weekly_rotation_correct_person() {
        let users = make_users(3);
        let sched = Schedule::new(
            "team".into(),
            zurich(),
            Rotation::Weekly,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        let on_call = sched.who_is_on_call(ts("2025-01-15T14:00:00Z"));
        assert!(users.contains(&on_call));
    }

    #[test]
    fn daily_rotation_correct_person() {
        let users = make_users(2);
        let sched = Schedule::new(
            "daily".into(),
            zurich(),
            Rotation::Daily,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        let day1 = sched.who_is_on_call(ts("2025-01-15T10:00:00Z"));
        let day2 = sched.who_is_on_call(ts("2025-01-16T10:00:00Z"));
        // Different days should give different people with 2 participants
        assert_ne!(day1, day2);
    }

    #[test]
    fn override_takes_precedence() {
        let users = make_users(2);
        let mut sched = Schedule::new(
            "team".into(),
            zurich(),
            Rotation::Weekly,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        let override_user = UserId::new();
        let ovr = ScheduleOverride::new(
            override_user.clone(),
            ts("2025-01-14T00:00:00Z"),
            ts("2025-01-15T00:00:00Z"),
        );
        sched.add_override(ovr, ts("2025-01-13T00:00:00Z")).unwrap();

        assert_eq!(
            sched.who_is_on_call(ts("2025-01-14T10:00:00Z")),
            override_user
        );
    }

    #[test]
    fn override_expires_rotation_resumes() {
        let users = make_users(2);
        let mut sched = Schedule::new(
            "team".into(),
            zurich(),
            Rotation::Weekly,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        let override_user = UserId::new();
        let ovr = ScheduleOverride::new(
            override_user,
            ts("2025-01-14T00:00:00Z"),
            ts("2025-01-15T00:00:00Z"),
        );
        sched.add_override(ovr, ts("2025-01-13T00:00:00Z")).unwrap();

        // After override expires, rotation resumes
        let on_call = sched.who_is_on_call(ts("2025-01-15T10:00:00Z"));
        assert!(users.contains(&on_call));
    }

    #[test]
    fn rotation_wraps_around() {
        let users = make_users(3);
        let sched = Schedule::new(
            "wrap".into(),
            zurich(),
            Rotation::Daily,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        // Check 4 consecutive days — day 4 should wrap to same as day 1
        let day1 = sched.who_is_on_call(ts("2025-01-15T10:00:00Z"));
        let day4 = sched.who_is_on_call(ts("2025-01-18T10:00:00Z"));
        assert_eq!(day1, day4);
    }

    #[test]
    fn timezone_aware_handoff() {
        let users = make_users(2);
        let sched = Schedule::new(
            "tz".into(),
            zurich(),
            Rotation::Daily,
            users.clone(),
            handoff_monday_9(),
        )
        .unwrap();

        // Same UTC time maps to same local time consistently
        let on_call = sched.who_is_on_call(ts("2025-01-15T08:00:00Z"));
        assert!(users.contains(&on_call));
    }

    #[test]
    fn add_override_with_invalid_period_fails() {
        let users = make_users(1);
        let mut sched = Schedule::new(
            "test".into(),
            zurich(),
            Rotation::Weekly,
            users,
            handoff_monday_9(),
        )
        .unwrap();

        let ovr = ScheduleOverride::new(
            UserId::new(),
            ts("2025-01-15T10:00:00Z"),
            ts("2025-01-15T09:00:00Z"), // end before start
        );
        let result = sched.add_override(ovr, ts("2025-01-14T00:00:00Z"));
        assert_eq!(result, Err(DomainError::InvalidOverridePeriod));
    }

    #[test]
    fn remove_override_returns_event() {
        let users = make_users(1);
        let mut sched = Schedule::new(
            "test".into(),
            zurich(),
            Rotation::Weekly,
            users,
            handoff_monday_9(),
        )
        .unwrap();

        let ovr = ScheduleOverride::new(
            UserId::new(),
            ts("2025-01-14T00:00:00Z"),
            ts("2025-01-16T00:00:00Z"),
        );
        let ovr_id = ovr.id().clone();
        sched.add_override(ovr, ts("2025-01-13T00:00:00Z")).unwrap();

        let events = sched
            .remove_override(&ovr_id, ts("2025-01-14T10:00:00Z"))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "oncall.changed");
    }

    #[test]
    fn remove_nonexistent_override_is_noop() {
        let users = make_users(1);
        let mut sched = Schedule::new(
            "test".into(),
            zurich(),
            Rotation::Weekly,
            users,
            handoff_monday_9(),
        )
        .unwrap();

        let fake_id = OverrideId::new();
        let events = sched
            .remove_override(&fake_id, ts("2025-01-14T10:00:00Z"))
            .unwrap();
        assert!(events.is_empty());
    }
}
