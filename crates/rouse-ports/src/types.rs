use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use rouse_core::alert::Severity;
use rouse_core::alert::Status;
use rouse_core::channel::Channel;
use rouse_core::ids::{AlertId, PolicyId};

/// Raw alert data from an external source, before domain validation.
#[derive(Debug, Clone)]
pub struct RawAlert {
    pub external_id: String,
    pub source: String,
    pub severity: String,
    pub labels: BTreeMap<String, String>,
    pub summary: String,
    pub status: String,
}

/// Notification ready to be sent via a channel adapter.
#[derive(Debug, Clone)]
pub struct Notification {
    pub alert_id: AlertId,
    pub severity: Severity,
    pub summary: String,
    pub labels: BTreeMap<String, String>,
    pub target: String,
    pub base_url: String,
}

/// Delivery metadata returned by notifiers.
#[derive(Debug, Clone, Default)]
pub struct NotifyResult {
    pub external_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Filter criteria for querying alerts.
#[derive(Debug, Clone, Default)]
pub struct AlertFilter {
    pub status: Option<Status>,
    pub severity: Option<Severity>,
    pub source: Option<String>,
    pub search: Option<String>,
    pub page: u32,
    pub per_page: u32,
}

/// A notification waiting in the database queue.
#[derive(Debug, Clone)]
pub struct PendingNotification {
    pub id: String,
    pub alert_id: AlertId,
    pub channel: Channel,
    pub target: String,
    pub payload: String,
    pub status: QueueStatus,
    pub next_attempt_at: DateTime<Utc>,
    pub retry_count: u32,
    pub created_at: DateTime<Utc>,
}

/// An escalation step waiting to fire.
#[derive(Debug, Clone)]
pub struct PendingEscalation {
    pub id: String,
    pub alert_id: AlertId,
    pub policy_id: PolicyId,
    pub step_order: u32,
    pub fires_at: DateTime<Utc>,
    pub status: QueueStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueueStatus {
    Pending,
    Sent,
    Failed,
    Dead,
}
