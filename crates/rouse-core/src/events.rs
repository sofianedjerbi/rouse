use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::alert::severity::Severity;
use crate::ids::{AlertId, ScheduleId, UserId};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum DomainEvent {
    AlertReceived(AlertReceived),
    AlertAcknowledged(AlertAcknowledged),
    AlertResolved(AlertResolved),
    OnCallChanged(OnCallChanged),
}

impl DomainEvent {
    pub fn occurred_at(&self) -> DateTime<Utc> {
        match self {
            Self::AlertReceived(e) => e.occurred_at,
            Self::AlertAcknowledged(e) => e.occurred_at,
            Self::AlertResolved(e) => e.occurred_at,
            Self::OnCallChanged(e) => e.occurred_at,
        }
    }

    pub fn event_type(&self) -> &'static str {
        match self {
            Self::AlertReceived(_) => "alert.received",
            Self::AlertAcknowledged(_) => "alert.acknowledged",
            Self::AlertResolved(_) => "alert.resolved",
            Self::OnCallChanged(_) => "oncall.changed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertReceived {
    pub alert_id: AlertId,
    pub source: String,
    pub severity: Severity,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertAcknowledged {
    pub alert_id: AlertId,
    pub user_id: UserId,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AlertResolved {
    pub alert_id: AlertId,
    pub resolved_by: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OnCallChanged {
    pub schedule_id: ScheduleId,
    pub new_user: UserId,
    pub previous_user: Option<UserId>,
    pub occurred_at: DateTime<Utc>,
}
