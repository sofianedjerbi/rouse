use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use rouse_core::alert::group::AlertGroup;
use rouse_core::alert::noise::NoiseScore;
use rouse_core::alert::Alert;
use rouse_core::channel::Channel;
use rouse_core::escalation::EscalationPolicy;
use rouse_core::events::DomainEvent;
use rouse_core::schedule::Schedule;

use crate::error::{NotifyError, ParseError, PortError};
use crate::types::{
    AlertFilter, Notification, NotifyResult, PendingEscalation, PendingNotification, RawAlert,
};

#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, notification: &Notification) -> Result<NotifyResult, NotifyError>;
    fn channel(&self) -> Channel;
}

#[async_trait]
pub trait AlertRepository: Send + Sync {
    async fn save(&self, alert: &Alert) -> Result<(), PortError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Alert>, PortError>;
    async fn find_by_fingerprint(&self, fp: &str) -> Result<Option<Alert>, PortError>;
    async fn find_by_filter(&self, filter: &AlertFilter) -> Result<Vec<Alert>, PortError>;
}

#[async_trait]
pub trait ScheduleRepository: Send + Sync {
    async fn save(&self, schedule: &Schedule) -> Result<(), PortError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<Schedule>, PortError>;
    async fn list_all(&self) -> Result<Vec<Schedule>, PortError>;
}

#[async_trait]
pub trait EscalationRepository: Send + Sync {
    async fn save(&self, policy: &EscalationPolicy) -> Result<(), PortError>;
    async fn find_by_id(&self, id: &str) -> Result<Option<EscalationPolicy>, PortError>;
}

#[async_trait]
pub trait NotificationQueue: Send + Sync {
    async fn enqueue(&self, notification: PendingNotification) -> Result<(), PortError>;
    async fn poll_pending(&self) -> Result<Vec<PendingNotification>, PortError>;
    async fn mark_sent(&self, id: &str) -> Result<(), PortError>;
    async fn mark_failed(
        &self,
        id: &str,
        error: &str,
        next_attempt: DateTime<Utc>,
    ) -> Result<(), PortError>;
    async fn mark_dead(&self, id: &str) -> Result<(), PortError>;
}

#[async_trait]
pub trait EscalationQueue: Send + Sync {
    async fn enqueue_step(&self, step: PendingEscalation) -> Result<(), PortError>;
    async fn poll_due(&self) -> Result<Vec<PendingEscalation>, PortError>;
    async fn cancel_for_alert(&self, alert_id: &str) -> Result<(), PortError>;
    async fn mark_fired(&self, id: &str) -> Result<(), PortError>;
}

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, events: Vec<DomainEvent>) -> Result<(), PortError>;
}

#[async_trait]
pub trait AlertGroupRepository: Send + Sync {
    async fn save(&self, group: &AlertGroup) -> Result<(), PortError>;
    async fn find_active_by_key(&self, key: &str) -> Result<Option<AlertGroup>, PortError>;
}

#[async_trait]
pub trait NoiseRepository: Send + Sync {
    async fn get_or_create(&self, fingerprint: &str) -> Result<NoiseScore, PortError>;
    async fn save(&self, score: &NoiseScore) -> Result<(), PortError>;
    async fn get_noisiest(&self, min_fires: u64) -> Result<Vec<NoiseScore>, PortError>;
}

pub trait AlertSourceParser: Send + Sync {
    fn parse(
        &self,
        payload: &[u8],
        headers: &HashMap<String, String>,
    ) -> Result<Vec<RawAlert>, ParseError>;
    fn source_name(&self) -> &str;
}
