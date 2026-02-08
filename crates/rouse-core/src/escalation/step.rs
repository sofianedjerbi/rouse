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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::UserId;

    #[test]
    fn step_preserves_order_and_wait() {
        let step = EscalationStep::new(
            2,
            300,
            vec![EscalationTarget::User(UserId::new())],
            vec![Channel::Slack],
        );
        assert_eq!(step.order(), 2);
        assert_eq!(step.wait_seconds(), 300);
    }

    #[test]
    fn step_preserves_targets_and_channels() {
        let step = EscalationStep::new(
            0,
            0,
            vec![EscalationTarget::User(UserId::new())],
            vec![Channel::Slack, Channel::Sms],
        );
        assert_eq!(step.targets().len(), 1);
        assert_eq!(step.channels().len(), 2);
    }
}
