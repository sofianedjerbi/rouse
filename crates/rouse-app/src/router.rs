use std::collections::BTreeMap;

use rouse_core::ids::PolicyId;

pub struct Route {
    pub matchers: BTreeMap<String, String>,
    pub policy_id: PolicyId,
}

pub struct AlertRouter {
    routes: Vec<Route>,
}

impl AlertRouter {
    pub fn new(routes: Vec<Route>) -> Self {
        Self { routes }
    }

    pub fn match_alert(&self, labels: &BTreeMap<String, String>) -> Option<&PolicyId> {
        self.routes.iter().find_map(|route| {
            let all_match = route.matchers.iter().all(|(k, v)| labels.get(k) == Some(v));
            if all_match {
                Some(&route.policy_id)
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn router_matches_first_route() {
        let policy_a = PolicyId::new();
        let policy_b = PolicyId::new();
        let router = AlertRouter::new(vec![
            Route {
                matchers: BTreeMap::from([("service".into(), "api".into())]),
                policy_id: policy_a.clone(),
            },
            Route {
                matchers: BTreeMap::from([("service".into(), "web".into())]),
                policy_id: policy_b,
            },
        ]);

        let labels = BTreeMap::from([
            ("service".into(), "api".into()),
            ("env".into(), "prod".into()),
        ]);
        assert_eq!(router.match_alert(&labels), Some(&policy_a));
    }

    #[test]
    fn router_no_match_returns_none() {
        let router = AlertRouter::new(vec![Route {
            matchers: BTreeMap::from([("service".into(), "api".into())]),
            policy_id: PolicyId::new(),
        }]);

        let labels = BTreeMap::from([("service".into(), "unknown".into())]);
        assert_eq!(router.match_alert(&labels), None);
    }

    #[test]
    fn router_requires_all_matchers() {
        let router = AlertRouter::new(vec![Route {
            matchers: BTreeMap::from([
                ("service".into(), "api".into()),
                ("env".into(), "prod".into()),
            ]),
            policy_id: PolicyId::new(),
        }]);

        // Only one matcher matches — should not match
        let labels = BTreeMap::from([("service".into(), "api".into())]);
        assert_eq!(router.match_alert(&labels), None);
    }

    #[test]
    fn router_empty_matchers_matches_everything() {
        let policy = PolicyId::new();
        let router = AlertRouter::new(vec![Route {
            matchers: BTreeMap::new(),
            policy_id: policy.clone(),
        }]);

        let labels = BTreeMap::from([("anything".into(), "here".into())]);
        assert_eq!(router.match_alert(&labels), Some(&policy));
    }
}
