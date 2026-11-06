use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

/// `[retention]`: how many old deployments ricochet keeps, and how large a bundle may be.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeploymentSettings {
    pub max_deployments: String,
    pub max_age_days: String,
    pub max_bundle_size: String,
}

impl SettingsSection for DeploymentSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let retention = table.child("retention");
        let number = |key: &str| {
            retention
                .and_then(|retention| retention.int(key))
                .map(|n| n.to_string())
                .unwrap_or_default()
        };
        Self {
            max_deployments: number("max_deployments"),
            max_age_days: number("max_age_days"),
            max_bundle_size: retention
                .and_then(|retention| {
                    retention
                        .string("max_bundle_size")
                        .or_else(|| retention.int("max_bundle_size").map(|n| n.to_string()))
                })
                .unwrap_or_default(),
        }
    }

    fn apply(&self, table: &mut Table) {
        let retention = table.section("retention");
        retention.set("max_deployments", self.max_deployments.whole_number());
        retention.set("max_age_days", self.max_age_days.whole_number());
        retention.set("max_bundle_size", self.max_bundle_size.non_empty());
        table.remove_if_empty("retention");
    }

    fn problem(&self) -> Option<String> {
        if !self.max_deployments.is_empty_or_positive() {
            return Some("Deployment history must be a whole number.".into());
        }
        if !self.max_age_days.is_empty_or_positive() {
            return Some("Deployment retention must be a whole number of days.".into());
        }
        if let Some(size) = self.max_bundle_size.non_empty() {
            let number: String = size
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let unit = size[number.len()..].trim().to_lowercase();
            const UNITS: [&str; 10] = ["", "b", "kb", "mb", "gb", "tb", "kib", "mib", "gib", "tib"];
            if number.parse::<f64>().is_err() || !UNITS.contains(&unit.as_str()) {
                return Some("Max bundle size must be a size, such as 100MB or 1GB.".into());
            }
        }
        None
    }
}
