use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

/// `[tasks]` and `[restore]`: how long a run or a package install may take, in minutes on screen and seconds on disk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TaskSettings {
    pub max_duration: String,
    pub restore_timeout: String,
    pub restore_concurrency: String,
}

fn minutes(seconds: Option<i64>) -> String {
    match seconds {
        None => String::new(),
        Some(seconds) if seconds % 60 == 0 => (seconds / 60).to_string(),
        Some(seconds) => (seconds as f64 / 60.0).to_string(),
    }
}

fn seconds(minutes: &str) -> Option<i64> {
    minutes
        .non_empty()?
        .parse::<f64>()
        .ok()
        .map(|minutes| (minutes * 60.0).round() as i64)
}

impl SettingsSection for TaskSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let restore = table.child("restore");
        Self {
            max_duration: minutes(
                table
                    .child("tasks")
                    .and_then(|tasks| tasks.int("max_duration")),
            ),
            restore_timeout: minutes(restore.and_then(|restore| restore.int("timeout"))),
            restore_concurrency: restore
                .and_then(|restore| restore.int("concurrency"))
                .map(|n| n.to_string())
                .unwrap_or_default(),
        }
    }

    fn apply(&self, table: &mut Table) {
        table
            .section("tasks")
            .set("max_duration", seconds(&self.max_duration));
        table.remove_if_empty("tasks");
        let restore = table.section("restore");
        restore.set("timeout", seconds(&self.restore_timeout));
        restore.set("concurrency", self.restore_concurrency.whole_number());
        table.remove_if_empty("restore");
    }

    fn problem(&self) -> Option<String> {
        for entered in [&self.max_duration, &self.restore_timeout] {
            if entered.non_empty().is_some()
                && !seconds(entered).is_some_and(|s| s > 0 && s <= i64::from(u32::MAX))
            {
                return Some("Timeouts must be whole minutes above 0.".into());
            }
        }
        if let Some(concurrency) = self.restore_concurrency.non_empty()
            && concurrency.parse::<u16>().map_or(true, |n| n == 0)
        {
            return Some("Concurrent package installs must be between 1 and 65535.".into());
        }
        None
    }
}
