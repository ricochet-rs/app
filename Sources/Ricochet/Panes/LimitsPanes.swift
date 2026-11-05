import SwiftUI

struct DeploymentsPane: View {
    @Binding var settings: DeploymentSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            InputField(
                "Deployments to keep", text: $settings.maxDeployments, prompt: "5",
                description: "Ricochet keeps this many deployments of each item. Leave this empty for 5.")
            InputField(
                "Delete deployments after", text: $settings.maxAgeDays, prompt: "Days",
                description: "In days. Leave this empty and ricochet keeps deployments however old they get.")
            InputField(
                "Largest bundle", text: $settings.maxBundleSize, prompt: "100MB",
                description: "Ricochet turns away bigger bundles. Leave this empty for 100MB.")
        }
    }
}

struct TasksPane: View {
    @Binding var settings: TaskSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            InputField(
                "Longest task run", text: $settings.maxDuration, prompt: "60",
                description: "In minutes. Ricochet stops a run that takes longer. Leave this empty for 60.")
            InputField(
                "Longest package install", text: $settings.restoreTimeout, prompt: "Minutes",
                description: "In minutes. Ricochet stops an install that takes longer. Leave this empty for ricochet's default.")
            InputField(
                "Package installs at once", text: $settings.restoreConcurrency, prompt: "4",
                description: "Ricochet runs this many installs at the same time. Leave this empty for 4.")
        }
    }
}

struct LoggingPane: View {
    @Binding var settings: LoggingSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SelectField(
                "Level", selection: $settings.level, options: LogLevel.allCases,
                description: "Ricochet logs far more at Debug and Trace than at Info, the default."
            ) { $0.title }
            SelectField("Start a new file", selection: $settings.rotation, options: LogRotation.allCases) {
                $0.title
            }
            ToggleField("Compress old files", isOn: $settings.compress)
            InputField(
                "Files to keep", text: $settings.maxFiles, prompt: "All",
                description: "Leave this empty and ricochet keeps every file.")
            InputField(
                "Delete files after", text: $settings.maxAgeDays, prompt: "Days",
                description: "In days. Leave this empty and ricochet keeps files however old they get.")
        }
    }
}

struct AuditLogPane: View {
    @Binding var settings: AuditLogSettings

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ToggleField("Keep records forever", isOn: $settings.keepForever)
            if !settings.keepForever {
                InputField(
                    "Keep records for", text: $settings.retentionDays, prompt: "365",
                    description: "In days. Leave this empty for 365.")
            }
            ToggleField(
                "Also write a log file", isOn: $settings.writeFile,
                description: "Ricochet copies each record to logs/audit/audit.log.")
        }
    }
}

struct TelemetryPane: View {
    @Binding var settings: TelemetrySettings

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            ToggleField("Send traces", isOn: $settings.enabled)
            if settings.enabled {
                InputField("Collector URL", text: $settings.endpoint, prompt: "http://localhost:4317")
            }
        }
    }
}
