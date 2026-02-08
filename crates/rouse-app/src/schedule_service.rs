use chrono::{DateTime, Utc};

use rouse_core::ids::{OverrideId, ScheduleId, UserId};
use rouse_core::schedule::{Schedule, ScheduleOverride};
use rouse_ports::error::PortError;
use rouse_ports::outbound::{EventPublisher, ScheduleRepository};

use crate::error::AppError;

pub struct ScheduleService<S, EP>
where
    S: ScheduleRepository,
    EP: EventPublisher,
{
    schedules: S,
    events: EP,
}

impl<S, EP> ScheduleService<S, EP>
where
    S: ScheduleRepository,
    EP: EventPublisher,
{
    pub fn new(schedules: S, events: EP) -> Self {
        Self { schedules, events }
    }

    pub async fn create_schedule(&self, schedule: Schedule) -> Result<ScheduleId, AppError> {
        let id = schedule.id().clone();
        self.schedules.save(&schedule).await?;
        Ok(id)
    }

    pub async fn who_is_on_call(
        &self,
        schedule_id: &str,
        at: DateTime<Utc>,
    ) -> Result<UserId, AppError> {
        let schedule = self
            .schedules
            .find_by_id(schedule_id)
            .await?
            .ok_or(AppError::Port(PortError::NotFound))?;
        Ok(schedule.who_is_on_call(at))
    }

    pub async fn add_override(
        &self,
        schedule_id: &str,
        ovr: ScheduleOverride,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut schedule = self
            .schedules
            .find_by_id(schedule_id)
            .await?
            .ok_or(AppError::Port(PortError::NotFound))?;

        let events = schedule.add_override(ovr, now)?;
        self.schedules.save(&schedule).await?;
        self.events.publish(events).await?;

        Ok(())
    }

    pub async fn remove_override(
        &self,
        schedule_id: &str,
        override_id: &str,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut schedule = self
            .schedules
            .find_by_id(schedule_id)
            .await?
            .ok_or(AppError::Port(PortError::NotFound))?;

        let ovr_id = OverrideId::parse(override_id)?;
        let events = schedule.remove_override(&ovr_id, now)?;

        if !events.is_empty() {
            self.schedules.save(&schedule).await?;
            self.events.publish(events).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rouse_core::events::DomainEvent;
    use rouse_core::schedule::{HandoffTime, Rotation};
    use rouse_ports::error::PortError;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockScheduleRepo {
        schedules: Mutex<Vec<Schedule>>,
    }

    #[async_trait]
    impl ScheduleRepository for MockScheduleRepo {
        async fn save(&self, schedule: &Schedule) -> Result<(), PortError> {
            let mut schedules = self.schedules.lock().unwrap();
            if let Some(pos) = schedules.iter().position(|s| s.id() == schedule.id()) {
                schedules[pos] = schedule.clone();
            } else {
                schedules.push(schedule.clone());
            }
            Ok(())
        }
        async fn find_by_id(&self, id: &str) -> Result<Option<Schedule>, PortError> {
            let schedules = self.schedules.lock().unwrap();
            Ok(schedules.iter().find(|s| s.id().to_string() == id).cloned())
        }
        async fn list_all(&self) -> Result<Vec<Schedule>, PortError> {
            Ok(self.schedules.lock().unwrap().clone())
        }
    }

    #[derive(Default)]
    struct MockEventPublisher {
        events: Mutex<Vec<DomainEvent>>,
    }

    #[async_trait]
    impl EventPublisher for MockEventPublisher {
        async fn publish(&self, events: Vec<DomainEvent>) -> Result<(), PortError> {
            self.events.lock().unwrap().extend(events);
            Ok(())
        }
    }

    fn zurich() -> chrono_tz::Tz {
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

    fn make_service() -> ScheduleService<MockScheduleRepo, MockEventPublisher> {
        ScheduleService::new(MockScheduleRepo::default(), MockEventPublisher::default())
    }

    fn make_schedule(users: Vec<UserId>) -> Schedule {
        Schedule::new(
            "platform".into(),
            zurich(),
            Rotation::Weekly,
            users,
            handoff_monday_9(),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn create_schedule_saves() {
        let svc = make_service();
        let users = make_users(3);
        let schedule = make_schedule(users);
        let schedule_id = schedule.id().clone();

        let result = svc.create_schedule(schedule).await.unwrap();
        assert_eq!(result, schedule_id);

        let schedules = svc.schedules.schedules.lock().unwrap();
        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].id(), &schedule_id);
    }

    #[tokio::test]
    async fn who_is_on_call_delegates_to_domain() {
        let svc = make_service();
        let users = make_users(3);
        let schedule = make_schedule(users.clone());
        let schedule_id = schedule.id().clone();

        svc.create_schedule(schedule).await.unwrap();

        let on_call = svc
            .who_is_on_call(&schedule_id.to_string(), ts("2025-01-15T14:00:00Z"))
            .await
            .unwrap();

        assert!(users.contains(&on_call));
    }

    #[tokio::test]
    async fn add_override_persists_and_publishes() {
        let svc = make_service();
        let users = make_users(2);
        let schedule = make_schedule(users);
        let schedule_id = schedule.id().clone();

        svc.create_schedule(schedule).await.unwrap();

        let override_user = UserId::new();
        let ovr = ScheduleOverride::new(
            override_user.clone(),
            ts("2025-01-14T00:00:00Z"),
            ts("2025-01-15T00:00:00Z"),
        );

        svc.add_override(&schedule_id.to_string(), ovr, ts("2025-01-13T00:00:00Z"))
            .await
            .unwrap();

        // Verify override affects on-call query
        let on_call = svc
            .who_is_on_call(&schedule_id.to_string(), ts("2025-01-14T10:00:00Z"))
            .await
            .unwrap();
        assert_eq!(on_call, override_user);

        // Verify event published
        let events = svc.events.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "oncall.changed");
    }
}
