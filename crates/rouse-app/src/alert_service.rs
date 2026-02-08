use chrono::{DateTime, Utc};

use rouse_core::alert::{Alert, Fingerprint, Severity, Source};
use rouse_core::events::{AlertDeduplicated, DomainEvent};
use rouse_core::ids::{AlertId, UserId};
use rouse_ports::error::PortError;
use rouse_ports::outbound::{AlertRepository, EscalationQueue, EventPublisher};
use rouse_ports::types::RawAlert;

use crate::error::AppError;
use crate::router::AlertRouter;

pub struct AlertService<A, EQ, EP>
where
    A: AlertRepository,
    EQ: EscalationQueue,
    EP: EventPublisher,
{
    alerts: A,
    escalation_queue: EQ,
    events: EP,
    router: AlertRouter,
}

impl<A, EQ, EP> AlertService<A, EQ, EP>
where
    A: AlertRepository,
    EQ: EscalationQueue,
    EP: EventPublisher,
{
    pub fn new(alerts: A, escalation_queue: EQ, events: EP, router: AlertRouter) -> Self {
        Self {
            alerts,
            escalation_queue,
            events,
            router,
        }
    }

    pub async fn receive(&self, raw: RawAlert, now: DateTime<Utc>) -> Result<AlertId, AppError> {
        let labels = raw.labels.clone();
        let fingerprint = Fingerprint::from_labels(&labels);

        // Source-initiated resolve
        if raw.status.to_lowercase() == "resolved" {
            let mut alert = self
                .alerts
                .find_by_fingerprint(fingerprint.as_str())
                .await?
                .ok_or(AppError::Port(PortError::NotFound))?;
            let alert_id = alert.id().clone();
            let resolved_by = format!("source:{}", raw.source);
            let events = alert.resolve(resolved_by, now)?;
            if !events.is_empty() {
                self.escalation_queue
                    .cancel_for_alert(&alert_id.to_string())
                    .await?;
                self.alerts.save(&alert).await?;
                self.events.publish(events).await?;
            }
            return Ok(alert_id);
        }

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

    pub async fn acknowledge(
        &self,
        alert_id: &AlertId,
        user_id: UserId,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut alert = self
            .alerts
            .find_by_id(&alert_id.to_string())
            .await?
            .ok_or(AppError::Port(PortError::NotFound))?;

        let events = alert.acknowledge(user_id, now)?;

        if events.is_empty() {
            return Ok(());
        }

        // TODO: wrap cancel+save+publish in a transaction once adapter supports it
        self.escalation_queue
            .cancel_for_alert(&alert_id.to_string())
            .await?;
        self.alerts.save(&alert).await?;
        self.events.publish(events).await?;

        Ok(())
    }

    pub async fn resolve(
        &self,
        alert_id: &AlertId,
        resolved_by: String,
        now: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let mut alert = self
            .alerts
            .find_by_id(&alert_id.to_string())
            .await?
            .ok_or(AppError::Port(PortError::NotFound))?;

        let events = alert.resolve(resolved_by, now)?;

        if events.is_empty() {
            return Ok(());
        }

        self.escalation_queue
            .cancel_for_alert(&alert_id.to_string())
            .await?;
        self.alerts.save(&alert).await?;
        self.events.publish(events).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use rouse_core::alert::{Alert, Status};
    use rouse_core::error::DomainError;
    use rouse_core::events::DomainEvent;
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
            let mut alerts = self.alerts.lock().unwrap();
            if let Some(pos) = alerts.iter().position(|a| a.id() == alert.id()) {
                alerts[pos] = alert.clone();
            } else {
                alerts.push(alert.clone());
            }
            Ok(())
        }
        async fn find_by_id(&self, id: &str) -> Result<Option<Alert>, PortError> {
            let alerts = self.alerts.lock().unwrap();
            Ok(alerts.iter().find(|a| a.id().to_string() == id).cloned())
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
    struct MockEscalationQueue {
        cancelled: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl EscalationQueue for MockEscalationQueue {
        async fn enqueue_step(&self, _step: PendingEscalation) -> Result<(), PortError> {
            Ok(())
        }
        async fn poll_due(&self) -> Result<Vec<PendingEscalation>, PortError> {
            Ok(vec![])
        }
        async fn cancel_for_alert(&self, alert_id: &str) -> Result<(), PortError> {
            self.cancelled.lock().unwrap().push(alert_id.to_string());
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

    fn make_service() -> AlertService<MockAlertRepo, MockEscalationQueue, MockEventPublisher> {
        AlertService::new(
            MockAlertRepo::default(),
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

    #[tokio::test]
    async fn receive_resolved_unknown_fingerprint_returns_not_found() {
        let svc = make_service();
        let mut raw = make_raw_alert("api");
        raw.status = "resolved".into();

        let result = svc.receive(raw, now()).await;
        assert!(matches!(result, Err(AppError::Port(PortError::NotFound))));
    }

    #[tokio::test]
    async fn acknowledge_cancels_escalation() {
        let svc = make_service();
        let raw = make_raw_alert("api");
        let alert_id = svc.receive(raw, now()).await.unwrap();

        let user_id = UserId::new();
        svc.acknowledge(&alert_id, user_id, now()).await.unwrap();

        let alerts = svc.alerts.alerts.lock().unwrap();
        let alert = alerts.iter().find(|a| a.id() == &alert_id).unwrap();
        assert_eq!(alert.status(), Status::Acknowledged);

        let cancelled = svc.escalation_queue.cancelled.lock().unwrap();
        assert!(cancelled.contains(&alert_id.to_string()));

        let events = svc.events.events.lock().unwrap();
        assert!(events
            .iter()
            .any(|e| e.event_type() == "alert.acknowledged"));
    }

    #[tokio::test]
    async fn acknowledge_already_acknowledged_is_noop() {
        let svc = make_service();
        let raw = make_raw_alert("api");
        let alert_id = svc.receive(raw, now()).await.unwrap();

        let user_id = UserId::new();
        svc.acknowledge(&alert_id, user_id.clone(), now())
            .await
            .unwrap();

        let events_before = svc.events.events.lock().unwrap().len();
        let cancelled_before = svc.escalation_queue.cancelled.lock().unwrap().len();

        svc.acknowledge(&alert_id, user_id, now()).await.unwrap();

        let events_after = svc.events.events.lock().unwrap().len();
        let cancelled_after = svc.escalation_queue.cancelled.lock().unwrap().len();
        assert_eq!(events_before, events_after);
        assert_eq!(cancelled_before, cancelled_after);
    }

    #[tokio::test]
    async fn acknowledge_resolved_alert_fails() {
        let svc = make_service();
        let raw = make_raw_alert("api");
        let alert_id = svc.receive(raw, now()).await.unwrap();

        svc.resolve(&alert_id, "operator".into(), now())
            .await
            .unwrap();

        let result = svc.acknowledge(&alert_id, UserId::new(), now()).await;
        assert!(matches!(
            result,
            Err(AppError::Domain(DomainError::AlertAlreadyResolved))
        ));
    }

    #[tokio::test]
    async fn resolve_cancels_escalation() {
        let svc = make_service();
        let raw = make_raw_alert("api");
        let alert_id = svc.receive(raw, now()).await.unwrap();

        svc.resolve(&alert_id, "operator".into(), now())
            .await
            .unwrap();

        let alerts = svc.alerts.alerts.lock().unwrap();
        let alert = alerts.iter().find(|a| a.id() == &alert_id).unwrap();
        assert_eq!(alert.status(), Status::Resolved);

        let cancelled = svc.escalation_queue.cancelled.lock().unwrap();
        assert!(cancelled.contains(&alert_id.to_string()));

        let events = svc.events.events.lock().unwrap();
        assert!(events.iter().any(|e| e.event_type() == "alert.resolved"));
    }

    #[tokio::test]
    async fn resolve_by_source() {
        let svc = make_service();
        let raw = make_raw_alert("api");
        let alert_id = svc.receive(raw, now()).await.unwrap();

        // Source sends "resolved" for same fingerprint
        let mut resolve_raw = make_raw_alert("api");
        resolve_raw.status = "resolved".into();
        let resolved_id = svc.receive(resolve_raw, now()).await.unwrap();

        assert_eq!(alert_id, resolved_id);

        let alerts = svc.alerts.alerts.lock().unwrap();
        let alert = alerts.iter().find(|a| a.id() == &alert_id).unwrap();
        assert_eq!(alert.status(), Status::Resolved);

        let events = svc.events.events.lock().unwrap();
        let resolve_event = events
            .iter()
            .find(|e| e.event_type() == "alert.resolved")
            .unwrap();
        if let DomainEvent::AlertResolved(e) = resolve_event {
            assert_eq!(e.resolved_by, "source:alertmanager");
        } else {
            panic!("expected AlertResolved event");
        }
    }

    #[tokio::test]
    async fn resolve_already_resolved_is_noop() {
        let svc = make_service();
        let raw = make_raw_alert("api");
        let alert_id = svc.receive(raw, now()).await.unwrap();

        svc.resolve(&alert_id, "operator".into(), now())
            .await
            .unwrap();

        let events_before = svc.events.events.lock().unwrap().len();

        svc.resolve(&alert_id, "another".into(), now())
            .await
            .unwrap();

        let events_after = svc.events.events.lock().unwrap().len();
        assert_eq!(events_before, events_after);
    }
}
