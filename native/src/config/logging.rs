use super::choice::choice;
use super::section::{Field, SettingsSection, TableRead, TableWrite};
use toml_edit::{Table, TableLike};

choice! {
    LogLevel {
        Trace => ("trace", "Trace"),
        Debug => ("debug", "Debug"),
        Info => ("info", "Info"),
        Warn => ("warn", "Warning"),
        Error => ("error", "Error"),
    }
}

choice! {
    LogRotation {
        EveryMinute => ("everyminute", "Every minute"),
        Hourly => ("hourly", "Hourly"),
        Daily => ("daily", "Daily"),
        Never => ("never", "Never"),
    }
}

/// `[logs]`. File logging is always on, because the service manager never rotates the stdout file it writes.
#[derive(Clone, Debug, PartialEq)]
pub struct LoggingSettings {
    pub level: LogLevel,
    pub rotation: LogRotation,
    pub compress: bool,
    pub max_files: String,
    pub max_age_days: String,
}

impl Default for LoggingSettings {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            rotation: LogRotation::Daily,
            compress: true,
            max_files: String::new(),
            max_age_days: String::new(),
        }
    }
}

impl SettingsSection for LoggingSettings {
    fn from_table(table: &dyn TableLike) -> Self {
        let logs = table.child("logs");
        let number = |key: &str| {
            logs.and_then(|logs| logs.int(key))
                .map(|n| n.to_string())
                .unwrap_or_default()
        };
        Self {
            level: logs
                .and_then(|logs| logs.string("level"))
                .and_then(|raw| LogLevel::parse(&raw))
                .unwrap_or(LogLevel::Info),
            rotation: logs
                .and_then(|logs| logs.string("rotation"))
                .and_then(|raw| LogRotation::parse(&raw))
                .unwrap_or(LogRotation::Daily),
            compress: logs
                .and_then(|logs| logs.boolean("compress"))
                .unwrap_or(true),
            max_files: number("max_files"),
            max_age_days: number("max_age_days"),
        }
    }

    fn apply(&self, table: &mut Table) {
        let logs = table.section("logs");
        logs.set("file_logging", Some(true));
        logs.set(
            "level",
            (self.level != LogLevel::Info).then(|| self.level.raw()),
        );
        logs.set(
            "rotation",
            (self.rotation != LogRotation::Daily).then(|| self.rotation.raw()),
        );
        logs.set("compress", (!self.compress).then_some(false));
        logs.set("max_files", self.max_files.whole_number());
        logs.set("max_age_days", self.max_age_days.whole_number());
    }

    fn problem(&self) -> Option<String> {
        if !self.max_files.is_empty_or_positive() {
            return Some("Enter how many log files to keep as a whole number.".into());
        }
        if !self.max_age_days.is_empty_or_positive() {
            return Some("Enter the log age limit as a whole number of days.".into());
        }
        None
    }
}
