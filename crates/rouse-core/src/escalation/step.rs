use serde::{Deserialize, Serialize};

use crate::channel::Channel;

use super::target::EscalationTarget;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationStep {
    order: u32,
    wait_seconds: u64,
    targets: Vec<EscalationTarget>,
    channels: Vec<Channel>,
}

impl EscalationStep {
    pub fn new(
        order: u32,
        wait_seconds: u64,
        targets: Vec<EscalationTarget>,
        channels: Vec<Channel>,
    ) -> Self {
        Self {
            order,
            wait_seconds,
            targets,
            channels,
        }
    }

    pub fn order(&self) -> u32 {
        self.order
    }

    pub fn wait_seconds(&self) -> u64 {
        self.wait_seconds
    }

    pub fn targets(&self) -> &[EscalationTarget] {
        &self.targets
    }

    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }
}
