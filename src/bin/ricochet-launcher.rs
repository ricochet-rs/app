//! Starts ricochet-server for the background job: through `sandbox-exec` on macOS, and on Windows
//! with no console window, restarting it whenever it crashes.

#![cfg_attr(windows, windows_subsystem = "windows")]

/// Becomes `sandbox-exec` under the same pid, so launchd supervises the server while macOS names the job after this file.
#[cfg(target_os = "macos")]
fn main() {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new("/usr/bin/sandbox-exec")
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("/usr/bin/sandbox-exec: {error}");
    std::process::exit(1);
}

/// Runs `<program> <arguments>` in `<home>`, appending its output to `<stdout>` and writing its errors to `<stderr>`,
/// and starts it again after a crash, because Windows has no service manager for a per-user program.
#[cfg(windows)]
fn main() {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut arguments = std::env::args_os().skip(1);
    let (Some(home), Some(stdout), Some(stderr), Some(program)) = (
        arguments.next(),
        arguments.next(),
        arguments.next(),
        arguments.next(),
    ) else {
        std::process::exit(2);
    };
    let rest: Vec<_> = arguments.collect();
    loop {
        let open = |path: &std::ffi::OsStr, append: bool| {
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(append)
                .truncate(!append)
                .open(path)
        };
        let (Ok(out), Ok(err)) = (open(&stdout, true), open(&stderr, false)) else {
            std::process::exit(1);
        };
        let status = Command::new(&program)
            .args(&rest)
            .current_dir(&home)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(err)
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        match status {
            Ok(status) if status.success() => break,
            _ => std::thread::sleep(std::time::Duration::from_secs(5)),
        }
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn main() {
    eprintln!("ricochet-launcher only runs on macOS and Windows.");
    std::process::exit(1);
}
