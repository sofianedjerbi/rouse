use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint(String);

impl Fingerprint {
    pub fn from_labels(labels: &BTreeMap<String, String>) -> Self {
        let mut hasher = DefaultHasher::new();
        for (k, v) in labels {
            k.hash(&mut hasher);
            v.hash(&mut hasher);
        }
        Self(format!("{:016x}", hasher.finish()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_labels_produce_valid_fingerprint() {
        let fp = Fingerprint::from_labels(&BTreeMap::new());
        assert!(!fp.as_str().is_empty());
        assert_eq!(fp.as_str().len(), 16); // 16 hex chars
    }

    #[test]
    fn same_labels_produce_same_fingerprint() {
        let labels: BTreeMap<String, String> =
            BTreeMap::from([("a".into(), "1".into()), ("b".into(), "2".into())]);
        let fp1 = Fingerprint::from_labels(&labels);
        let fp2 = Fingerprint::from_labels(&labels);
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn different_labels_produce_different_fingerprint() {
        let a = BTreeMap::from([("a".into(), "1".into())]);
        let b = BTreeMap::from([("a".into(), "2".into())]);
        assert_ne!(Fingerprint::from_labels(&a), Fingerprint::from_labels(&b));
    }

    #[test]
    fn display_matches_as_str() {
        let fp = Fingerprint::from_labels(&BTreeMap::from([("k".into(), "v".into())]));
        assert_eq!(format!("{fp}"), fp.as_str());
    }
}
