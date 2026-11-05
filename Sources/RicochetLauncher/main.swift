import Darwin

// Becomes sandbox-exec under the same pid, so launchd supervises the server while macOS names the job after this file.
execv("/usr/bin/sandbox-exec", CommandLine.unsafeArgv)
perror("/usr/bin/sandbox-exec")
exit(1)
