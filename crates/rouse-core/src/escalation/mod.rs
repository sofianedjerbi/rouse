pub mod step;
pub mod target;

use serde::{Deserialize, Serialize};

use crate::error::DomainError;
use crate::events::DomainEvent;
use crate::ids::PolicyId;

pub use step::EscalationStep;
pub use target::{EscalationTarget, OnCallModifier};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationPolicy {
    id: PolicyId,
    name: String,
    steps: Vec<EscalationStep>,
    repeat_count: u32,
}

impl EscalationPolicy {
    pub fn new(
        name: String,
        steps: Vec<EscalationStep>,
        repeat_count: u32,
    ) -> Result<Self, DomainError> {
        if steps.is_empty() {
            return Err(DomainError::PolicyRequiresStep);
        }
        Ok(Self {
            id: PolicyId::new(),
            name,
            steps,
            repeat_count,
        })
    }

    pub fn next_step(&self, current: u32, repetition: u32) -> Option<&EscalationStep> {
        let next = current + 1;
        if (next as usize) < self.steps.len() {
            Some(&self.steps[next as usize])
        } else if repetition < self.repeat_count {
            Some(&self.steps[0])
        } else {
            None
        }
    }

    pub fn add_step(&mut self, step: EscalationStep) -> Result<Vec<DomainEvent>, DomainError> {
        if step.targets().is_empty() {
            return Err(DomainError::StepRequiresTarget);
        }
        if step.channels().is_empty() {
            return Err(DomainError::StepRequiresChannel);
        }
        self.steps.push(step);
        Ok(vec![])
    }

    pub fn first_step(&self) -> &EscalationStep {
        &self.steps[0]
    }

    pub fn id(&self) -> &PolicyId {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn steps(&self) -> &[EscalationStep] {
        &self.steps
    }

    pub fn repeat_count(&self) -> u32 {
        self.repeat_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::Channel;
    use crate::ids::{ScheduleId, UserId};

    fn make_step(order: u32, wait: u64) -> EscalationStep {
        EscalationStep::new(
            order,
            wait,
            vec![EscalationTarget::User(UserId::new())],
            vec![Channel::Slack],
        )
    }

    #[test]
    fn policy_requires_at_least_one_step() {
        let result = EscalationPolicy::new("empty".into(), vec![], 0);
        assert!(matches!(result, Err(DomainError::PolicyRequiresStep)));
    }

    #[test]
    fn step_zero_wait_is_zero() {
        let step = make_step(0, 0);
        let policy = EscalationPolicy::new("p".into(), vec![step], 0).unwrap();
        assert_eq!(policy.first_step().wait_seconds(), 0);
    }

    #[test]
    fn next_step_returns_correct() {
        let steps = vec![make_step(0, 0), make_step(1, 600)];
        let policy = EscalationPolicy::new("p".into(), steps, 0).unwrap();
        let next = policy.next_step(0, 0).unwrap();
        assert_eq!(next.order(), 1);
        assert_eq!(next.wait_seconds(), 600);
    }

    #[test]
    fn next_step_after_last_no_repeat_returns_none() {
        let steps = vec![make_step(0, 0), make_step(1, 600)];
        let policy = EscalationPolicy::new("p".into(), steps, 0).unwrap();
        assert!(policy.next_step(1, 0).is_none());
    }

    #[test]
    fn next_step_after_last_with_repeat_loops() {
        let steps = vec![make_step(0, 0), make_step(1, 600)];
        let policy = EscalationPolicy::new("p".into(), steps, 2).unwrap();
        let next = policy.next_step(1, 0).unwrap();
        assert_eq!(next.order(), 0); // loops back
    }

    #[test]
    fn repeat_exhausted_returns_none() {
        let steps = vec![make_step(0, 0)];
        let policy = EscalationPolicy::new("p".into(), steps, 1).unwrap();
        // repetition=1 means we've already repeated once, repeat_count=1
        assert!(policy.next_step(0, 1).is_none());
    }

    #[test]
    fn step_requires_target() {
        let mut policy = EscalationPolicy::new("p".into(), vec![make_step(0, 0)], 0).unwrap();
        let step = EscalationStep::new(1, 600, vec![], vec![Channel::Slack]);
        let result = policy.add_step(step);
        assert!(matches!(result, Err(DomainError::StepRequiresTarget)));
    }

    #[test]
    fn step_requires_channel() {
        let mut policy = EscalationPolicy::new("p".into(), vec![make_step(0, 0)], 0).unwrap();
        let step = EscalationStep::new(1, 600, vec![EscalationTarget::User(UserId::new())], vec![]);
        let result = policy.add_step(step);
        assert!(matches!(result, Err(DomainError::StepRequiresChannel)));
    }

    #[test]
    fn channel_enum_is_exhaustive() {
        // Verify all channel variants exist
        let channels = [
            Channel::Slack,
            Channel::Discord,
            Channel::Telegram,
            Channel::WhatsApp,
            Channel::Sms,
            Channel::Phone,
            Channel::Email,
            Channel::Webhook,
        ];
        assert_eq!(channels.len(), 8);
    }

    #[test]
    fn oncall_target_with_modifier() {
        let target = EscalationTarget::OnCall {
            schedule_id: ScheduleId::new(),
            modifier: OnCallModifier::Next,
        };
        assert!(matches!(
            target,
            EscalationTarget::OnCall {
                modifier: OnCallModifier::Next,
                ..
            }
        ));
    }
}
