use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

/// `[audit]`: how long ricochet keeps its record of who changed what.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuditLogSettings {
    pub keep_forever: bool,
    pub retention_days: String,
    pub write_file: bool,
}

impl SettingsSection for AuditLogSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let audit = table.child("audit");
        Self {
            keep_forever: audit
                .and_then(|audit| audit.string("retention_days"))
                .is_some_and(|saved| saved == "forever"),
            retention_days: audit
                .and_then(|audit| audit.int("retention_days"))
                .map(|n| n.to_string())
                .unwrap_or_default(),
            write_file: audit
                .and_then(|audit| audit.boolean("file_logging"))
                .unwrap_or(false),
        }
    }

    fn apply(&self, table: &mut Table) {
        let audit = table.section("audit");
        if self.keep_forever {
            audit.set("retention_days", Some("forever"));
        } else {
            audit.set("retention_days", self.retention_days.whole_number());
        }
        audit.set("file_logging", self.write_file.then_some(true));
        table.remove_if_empty("audit");
    }

    fn problem(&self) -> Option<String> {
        (!(self.keep_forever || self.retention_days.is_empty_or_positive()))
            .then(|| "Enter how long to keep audit records as a whole number of days.".into())
    }
}
