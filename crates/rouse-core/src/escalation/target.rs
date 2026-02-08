use serde::{Deserialize, Serialize};

use crate::ids::{ScheduleId, TeamId, UserId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscalationTarget {
    OnCall {
        schedule_id: ScheduleId,
        modifier: OnCallModifier,
    },
    User(UserId),
    Team(TeamId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OnCallModifier {
    Current,
    Next,
}
