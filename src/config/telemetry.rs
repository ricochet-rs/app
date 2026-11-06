use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

/// `[otel]`, the OpenTelemetry collector ricochet sends traces to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TelemetrySettings {
    pub enabled: bool,
    pub endpoint: String,
}

impl SettingsSection for TelemetrySettings {
    fn from_table(table: &dyn TableLike) -> Self {
        match table.child("otel").and_then(|otel| otel.string("endpoint")) {
            Some(endpoint) => Self {
                enabled: true,
                endpoint,
            },
            None => Self::default(),
        }
    }

    fn apply(&self, table: &mut Table) {
        if self.enabled {
            table
                .section("otel")
                .set("endpoint", Some(self.endpoint.trimmed()));
        } else {
            table.remove("otel");
        }
    }

    fn problem(&self) -> Option<String> {
        (self.enabled && !self.endpoint.is_http_url())
            .then(|| "Enter the collector's URL, such as http://localhost:4317.".into())
    }
}
