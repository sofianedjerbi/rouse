use async_trait::async_trait;
use chrono::{DateTime, Utc};

use rouse_core::alert::Alert;
use rouse_core::ids::{AlertId, UserId};
use rouse_core::schedule::Schedule;
use rouse_core::schedule::ScheduleOverride;

use crate::error::PortError;
use crate::types::{AlertFilter, RawAlert};

#[async_trait]
pub trait AlertReceiver: Send + Sync {
    async fn receive_alert(&self, raw: RawAlert) -> Result<AlertId, PortError>;
}

#[async_trait]
pub trait AlertManager: Send + Sync {
    async fn acknowledge(&self, alert_id: &str, user_id: &str) -> Result<(), PortError>;
    async fn resolve(&self, alert_id: &str, resolved_by: &str) -> Result<(), PortError>;
    async fn get_alert(&self, alert_id: &str) -> Result<Alert, PortError>;
    async fn list_alerts(&self, filter: AlertFilter) -> Result<Vec<Alert>, PortError>;
}

#[async_trait]
pub trait ScheduleManager: Send + Sync {
    async fn who_is_on_call(
        &self,
        schedule_id: &str,
        at: DateTime<Utc>,
    ) -> Result<UserId, PortError>;
    async fn create_schedule(&self, schedule: Schedule) -> Result<(), PortError>;
    async fn add_override(&self, schedule_id: &str, ovr: ScheduleOverride)
        -> Result<(), PortError>;
}
