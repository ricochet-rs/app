import Darwin
import TOMLKit

/// The ports ricochet-server listens on, chosen so none collides with another app or with each other.
struct NetworkPorts {
  let proxy: UInt16
  let api: UInt16
  let reload: UInt16
  let metrics: UInt16

  /// Keeps the ports already saved in `network`, because ricochet holds them itself,
  /// and probes for a free one only where none is saved.
  init(proxy: UInt16, keeping network: TOMLTable?) {
    var taken: Set<UInt16> = [proxy]
    func claim(_ saved: UInt16?, preferring preferred: UInt16) -> UInt16 {
      if let saved, !taken.contains(saved) {
        taken.insert(saved)
        return saved
      }
      let port =
        (preferred...preferred + 100).first { !taken.contains($0) && Self.isFree($0) } ?? preferred
      taken.insert(port)
      return port
    }
    self.proxy = proxy
    api = claim(network?["api_port"]?.int.flatMap { UInt16(exactly: $0) }, preferring: 3000)
    reload = claim(network?["reload_port"]?.int.flatMap { UInt16(exactly: $0) }, preferring: 3001)
    metrics = claim(
      network?["metrics"]?.string?.split(separator: ":").last.flatMap { UInt16($0) },
      preferring: 6189)
  }

  /// Whether ricochet could bind `port`, probed with `SO_REUSEADDR` as ricochet binds,
  /// so connections ricochet just closed don't make its own port look taken.
  static func isFree(_ port: UInt16) -> Bool {
    let fd = socket(AF_INET, SOCK_STREAM, 0)
    guard fd >= 0 else { return false }
    defer { close(fd) }
    var reuse: Int32 = 1
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &reuse, socklen_t(MemoryLayout<Int32>.size))
    var address = sockaddr_in()
    address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
    address.sin_family = sa_family_t(AF_INET)
    address.sin_port = port.bigEndian
    address.sin_addr = in_addr(s_addr: INADDR_ANY)
    return withUnsafePointer(to: &address) {
      $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
        bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) == 0
      }
    }
  }
}
