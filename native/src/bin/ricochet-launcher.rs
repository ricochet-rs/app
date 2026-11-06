//! Becomes `sandbox-exec` under the same pid, so launchd supervises the server while macOS names the job after this file.

#[cfg(target_os = "macos")]
fn main() {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new("/usr/bin/sandbox-exec")
        .args(std::env::args_os().skip(1))
        .exec();
    eprintln!("/usr/bin/sandbox-exec: {error}");
    std::process::exit(1);
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("ricochet-launcher only runs on macOS.");
    std::process::exit(1);
}
