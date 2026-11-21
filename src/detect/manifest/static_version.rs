use std::{cmp::Ordering, fmt};

use serde::Deserialize;

pub(super) const MANIFEST_ENGINE_VERSION: u32 = 3;

#[derive(Debug, Clone)]
pub(super) struct ManifestVersion(String);

impl ManifestVersion {
    pub(super) fn parse(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err("version must not be empty".to_string());
        }
        for segment in trimmed.split('.') {
            if segment.is_empty() {
                return Err(format!("version {trimmed:?} contains an empty segment"));
            }
            if !segment.chars().all(|ch| ch.is_ascii_digit()) {
                return Err(format!("version {trimmed:?} must be dotted numeric"));
            }
            segment
                .parse::<u64>()
                .map_err(|_| format!("version {trimmed:?} contains an oversized segment"))?;
        }
        Ok(Self(trimmed.to_string()))
    }
}

impl fmt::Display for ManifestVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ManifestVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl Ord for ManifestVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self.0.split('.');
        let mut right = other.0.split('.');
        loop {
            match (left.next(), right.next()) {
                (Some(left), Some(right)) => {
                    let left = left.parse::<u64>().unwrap_or(0);
                    let right = right.parse::<u64>().unwrap_or(0);
                    match left.cmp(&right) {
                        Ordering::Equal => {}
                        ordering => return ordering,
                    }
                }
                (Some(left), None) => {
                    if left.parse::<u64>().unwrap_or(0) != 0 {
                        return Ordering::Greater;
                    }
                }
                (None, Some(right)) => {
                    if right.parse::<u64>().unwrap_or(0) != 0 {
                        return Ordering::Less;
                    }
                }
                (None, None) => return Ordering::Equal,
            }
        }
    }
}

impl PartialOrd for ManifestVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ManifestVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ManifestVersion {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_versions_preserve_numeric_order_and_validation() {
        for invalid in ["", "1..2", "1.x", "1.18446744073709551616", "-1"] {
            assert!(ManifestVersion::parse(invalid).is_err(), "{invalid}");
        }
        let version = |value| ManifestVersion::parse(value).unwrap();
        assert_eq!(version("1"), version("1.0.00"));
        assert!(version("1.10") > version("1.9"));
        assert!(version("1.0.1") > version("1"));
        assert_eq!(version(" 2026.01.2 ").to_string(), "2026.01.2");
    }
}
