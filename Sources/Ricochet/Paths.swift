import Foundation

/// Where the app installs ricochet-server and where the server keeps its data.
enum Paths {
  static let userHome = FileManager.default.homeDirectoryForCurrentUser
  static let binary = userHome.appending(path: ".local/bin/ricochet-server")
  static let home = userHome.appending(path: ".local/share/ricochet")
  static let config = home.appending(path: "ricochet-config.toml")
  static let logs = home.appending(path: "logs")
  static let stdout = home.appending(path: "ricochet.stdout")
  static let stderr = home.appending(path: "ricochet.err")
  static let renv = home.appending(path: "vendor/r/renv.tar.gz")
  /// `HOME` for ricochet-server and its content, inside the one folder the sandbox lets them write.
  static let sandboxHome = home.appending(path: "home")

  /// The app's own files, outside every folder the sandbox lets content read.
  static let appSupport = userHome.appending(path: "Library/Application Support/Ricochet")
  /// Every failure the app showed, with the step it was on, so a cut-off message can still be read.
  static let appLog = appSupport.appending(path: "ricochet-app.log")
  /// The launch agent's executable, which macOS shows by name in **Login Items & Extensions**.
  static let launcher = appSupport.appending(path: "ricochet")
}
