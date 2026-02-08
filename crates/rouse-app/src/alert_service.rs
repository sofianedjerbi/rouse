use chrono::{DateTime, Utc};

use rouse_core::alert::{Alert, Fingerprint, Severity, Source};
use rouse_core::events::{AlertDeduplicated, DomainEvent};
use rouse_core::ids::AlertId;
use rouse_ports::outbound::{
    AlertRepository, EscalationQueue, EscalationRepository, EventPublisher, NotificationQueue,
    ScheduleRepository,
};
use rouse_ports::types::RawAlert;

use crate::error::AppError;
use crate::router::AlertRouter;

#[allow(dead_code)] // fields used in upcoming acknowledge/resolve methods
pub struct AlertService<A, S, E, NQ, EQ, EP>
where
    A: AlertRepository,
    S: ScheduleRepository,
    E: EscalationRepository,
    NQ: NotificationQueue,
    EQ: EscalationQueue,
    EP: EventPublisher,
{
    alerts: A,
    schedules: S,
    escalations: E,
    notification_queue: NQ,
    escalation_queue: EQ,
    events: EP,
    router: AlertRouter,
}

impl<A, S, E, NQ, EQ, EP> AlertService<A, S, E, NQ, EQ, EP>
where
    A: AlertRepository,
    S: ScheduleRepository,
    E: EscalationRepository,
    NQ: NotificationQueue,
    EQ: EscalationQueue,
    EP: EventPublisher,
{
    pub fn new(
        alerts: A,
        schedules: S,
        escalations: E,
        notification_queue: NQ,
        escalation_queue: EQ,
        events: EP,
        router: AlertRouter,
    ) -> Self {
        Self {
            alerts,
            schedules,
            escalations,
            notification_queue,
            escalation_queue,
            events,
            router,
        }
    }

    pub async fn receive(&self, raw: RawAlert, now: DateTime<Utc>) -> Result<AlertId, AppError> {
        let labels = raw.labels.clone();
        let fingerprint = Fingerprint::from_labels(&labels);

        // Dedup check
        if let Some(existing) = self
            .alerts
            .find_by_fingerprint(fingerprint.as_str())
            .await?
        {
            let existing_id = existing.id().clone();
            self.events
                .publish(vec![DomainEvent::AlertDeduplicated(AlertDeduplicated {
                    alert_id: existing_id.clone(),
                    fingerprint: fingerprint.to_string(),
                    occurred_at: now,
                })])
                .await?;
            return Ok(existing_id);
        }

        // Parse severity
        let severity = match raw.severity.to_lowercase().as_str() {
            "critical" => Severity::Critical,
            "warning" => Severity::Warning,
            _ => Severity::Info,
        };

        // Create alert
        let (alert, creation_events) = Alert::new(
            raw.external_id,
            Source::new(raw.source),
            severity,
            labels.clone(),
            raw.summary,
            now,
        );
        let alert_id = alert.id().clone();

        // Save
        self.alerts.save(&alert).await?;

        // Publish creation events
        self.events.publish(creation_events).await?;

        // Route — match labels to policy (best effort, no error if unmatched)
        if let Some(_policy_id) = self.router.match_alert(&labels) {
            // Escalation enqueuing will be handled when we have full policy resolution
            // For now, the routing match is recorded
        }

        Ok(alert_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rouse_core::alert::{Alert, Status};
    use rouse_core::escalation::EscalationPolicy;
    use rouse_core::events::DomainEvent;
    use rouse_core::schedule::Schedule;
    use rouse_ports::error::PortError;
    use rouse_ports::types::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    // --- Mock Adapters ---

    #[derive(Default)]
    struct MockAlertRepo {
        alerts: Mutex<Vec<Alert>>,
    }

    #[async_trait]
    impl AlertRepository for MockAlertRepo {
        async fn save(&self, alert: &Alert) -> Result<(), PortError> {
            self.alerts.lock().unwrap().push(alert.clone());
            Ok(())
        }
        async fn find_by_id(&self, _id: &str) -> Result<Option<Alert>, PortError> {
            Ok(None)
        }
        async fn find_by_fingerprint(&self, fp: &str) -> Result<Option<Alert>, PortError> {
            let alerts = self.alerts.lock().unwrap();
            Ok(alerts
                .iter()
                .find(|a| a.fingerprint().as_str() == fp)
                .cloned())
        }
        async fn find_by_filter(&self, _filter: &AlertFilter) -> Result<Vec<Alert>, PortError> {
            Ok(vec![])
        }
    }

    #[derive(Default)]
    struct MockScheduleRepo;

    #[async_trait]
    impl ScheduleRepository for MockScheduleRepo {
        async fn save(&self, _s: &Schedule) -> Result<(), PortError> {
            Ok(())
        }
        async fn find_by_id(&self, _id: &str) -> Result<Option<Schedule>, PortError> {
            Ok(None)
        }
        async fn list_all(&self) -> Result<Vec<Schedule>, PortError> {
            Ok(vec![])
        }
    }

    #[derive(Default)]
    struct MockEscalationRepo;

    #[async_trait]
    impl EscalationRepository for MockEscalationRepo {
        async fn save(&self, _p: &EscalationPolicy) -> Result<(), PortError> {
            Ok(())
        }
        async fn find_by_id(&self, _id: &str) -> Result<Option<EscalationPolicy>, PortError> {
            Ok(None)
        }
    }

    #[derive(Default)]
    struct MockNotificationQueue {
        items: Mutex<Vec<PendingNotification>>,
    }

    #[async_trait]
    impl NotificationQueue for MockNotificationQueue {
        async fn enqueue(&self, n: PendingNotification) -> Result<(), PortError> {
            self.items.lock().unwrap().push(n);
            Ok(())
        }
        async fn poll_pending(&self) -> Result<Vec<PendingNotification>, PortError> {
            Ok(vec![])
        }
        async fn mark_sent(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn mark_failed(
            &self,
            _id: &str,
            _error: &str,
            _next: DateTime<Utc>,
        ) -> Result<(), PortError> {
            Ok(())
        }
        async fn mark_dead(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct MockEscalationQueue {
        items: Mutex<Vec<PendingEscalation>>,
    }

    #[async_trait]
    impl EscalationQueue for MockEscalationQueue {
        async fn enqueue_step(&self, step: PendingEscalation) -> Result<(), PortError> {
            self.items.lock().unwrap().push(step);
            Ok(())
        }
        async fn poll_due(&self) -> Result<Vec<PendingEscalation>, PortError> {
            Ok(vec![])
        }
        async fn cancel_for_alert(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
        }
        async fn mark_fired(&self, _id: &str) -> Result<(), PortError> {
            Ok(())
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

    fn now() -> DateTime<Utc> {
        chrono::DateTime::parse_from_rfc3339("2025-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn make_raw_alert(service: &str) -> RawAlert {
        RawAlert {
            external_id: "ext-1".into(),
            source: "alertmanager".into(),
            severity: "critical".into(),
            labels: BTreeMap::from([("service".into(), service.into())]),
            summary: "High CPU".into(),
            status: "firing".into(),
        }
    }

    fn make_service() -> AlertService<
        MockAlertRepo,
        MockScheduleRepo,
        MockEscalationRepo,
        MockNotificationQueue,
        MockEscalationQueue,
        MockEventPublisher,
    > {
        AlertService::new(
            MockAlertRepo::default(),
            MockScheduleRepo::default(),
            MockEscalationRepo::default(),
            MockNotificationQueue::default(),
            MockEscalationQueue::default(),
            MockEventPublisher::default(),
            AlertRouter::new(vec![]),
        )
    }

    #[tokio::test]
    async fn receive_new_alert_saves_and_publishes_event() {
        let svc = make_service();
        let raw = make_raw_alert("api");

        let alert_id = svc.receive(raw, now()).await.unwrap();

        let alerts = svc.alerts.alerts.lock().unwrap();
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].id(), &alert_id);
        assert_eq!(alerts[0].status(), Status::Firing);

        let events = svc.events.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "alert.received");
    }

    #[tokio::test]
    async fn receive_duplicate_suppressed() {
        let svc = make_service();
        let raw1 = make_raw_alert("api");
        let raw2 = make_raw_alert("api"); // same labels = same fingerprint

        let id1 = svc.receive(raw1, now()).await.unwrap();
        let id2 = svc.receive(raw2, now()).await.unwrap();

        assert_eq!(id1, id2);

        let alerts = svc.alerts.alerts.lock().unwrap();
        assert_eq!(alerts.len(), 1); // only one saved

        let events = svc.events.events.lock().unwrap();
        assert_eq!(events.len(), 2); // AlertReceived + AlertDeduplicated
        assert_eq!(events[1].event_type(), "alert.deduplicated");
    }

    #[tokio::test]
    async fn receive_no_matching_policy_saved_not_routed() {
        use crate::router::Route;

        let svc = AlertService::new(
            MockAlertRepo::default(),
            MockScheduleRepo::default(),
            MockEscalationRepo::default(),
            MockNotificationQueue::default(),
            MockEscalationQueue::default(),
            MockEventPublisher::default(),
            AlertRouter::new(vec![Route {
                matchers: BTreeMap::from([("service".into(), "web".into())]),
                policy_id: rouse_core::ids::PolicyId::new(),
            }]),
        );
        let raw = make_raw_alert("api"); // won't match "web"

        svc.receive(raw, now()).await.unwrap();

        let alerts = svc.alerts.alerts.lock().unwrap();
        assert_eq!(alerts.len(), 1); // alert still saved
    }
}
