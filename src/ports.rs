use crate::config::section::TableRead;
use socket2::{Domain, Socket, Type};
use std::collections::HashSet;
use std::hash::{BuildHasher, RandomState};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::ops::RangeInclusive;
use toml_edit::{TableLike, value};

/// The ports IANA leaves for dynamic use, which no service claims by default.
const DYNAMIC: RangeInclusive<u16> = 49152..=65535;

/// The API and reload ports earlier versions saved, which other development servers often hold.
const OLD_DEFAULTS: [u16; 2] = [3000, 3001];

/// How many random ports to probe before taking one unprobed.
const ATTEMPTS: usize = 32;

/// The ports ricochet-server listens on, chosen so none collides with another app or with each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkPorts {
    pub proxy: u16,
    pub api: u16,
    pub reload: u16,
    pub metrics: u16,
}

impl NetworkPorts {
    /// Keeps the ports already saved in `network`, because ricochet may be holding them right now,
    /// and picks a free random one only where none is saved.
    pub fn new(proxy: u16, network: Option<&dyn TableLike>, is_free: &dyn Fn(u16) -> bool) -> Self {
        Self::choose(proxy, network, is_free, &|_| true)
    }

    /// Keeps a saved port only while it is free and not an old default, so call it only while ricochet is stopped.
    pub fn rechecked(
        proxy: u16,
        network: Option<&dyn TableLike>,
        is_free: &dyn Fn(u16) -> bool,
    ) -> Self {
        Self::choose(proxy, network, is_free, &|port| {
            !OLD_DEFAULTS.contains(&port) && is_free(port)
        })
    }

    /// The ports saved in `network`, or `None` when one is missing.
    pub fn saved(network: &dyn TableLike) -> Option<Self> {
        Some(Self {
            proxy: saved(Some(network), "proxy_port")?,
            api: saved(Some(network), "api_port")?,
            reload: saved(Some(network), "reload_port")?,
            metrics: saved_metrics(Some(network))?,
        })
    }

    /// Writes the ports into `network`, the `[network]` table of ricochet's config.
    pub fn write(&self, network: &mut dyn TableLike) {
        network.insert("proxy_port", value(i64::from(self.proxy)));
        network.insert("api_port", value(i64::from(self.api)));
        network.insert("reload_port", value(i64::from(self.reload)));
        network.insert("metrics", value(format!("127.0.0.1:{}", self.metrics)));
    }

    fn choose(
        proxy: u16,
        network: Option<&dyn TableLike>,
        is_free: &dyn Fn(u16) -> bool,
        keep: &dyn Fn(u16) -> bool,
    ) -> Self {
        let mut taken = HashSet::from([proxy]);
        let mut claim = |saved: Option<u16>| {
            let port = saved
                .filter(|port| !taken.contains(port) && keep(*port))
                .unwrap_or_else(|| random_free_port(&taken, is_free));
            taken.insert(port);
            port
        };
        Self {
            proxy,
            api: claim(saved(network, "api_port")),
            reload: claim(saved(network, "reload_port")),
            metrics: claim(saved_metrics(network)),
        }
    }

    /// Whether ricochet could bind `port`, probed as ricochet binds.
    pub fn is_free(port: u16) -> bool {
        let Ok(socket) = Socket::new(Domain::IPV4, Type::STREAM, None) else {
            return false;
        };
        // `SO_REUSEADDR` keeps connections ricochet just closed from making its own port look taken.
        // Windows would let it bind over another app's listener, and has no such wait to skip.
        #[cfg(not(windows))]
        let _ = socket.set_reuse_address(true);
        socket
            .bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())
            .is_ok()
    }
}

fn saved(network: Option<&dyn TableLike>, key: &str) -> Option<u16> {
    network
        .and_then(|n| n.int(key))
        .and_then(|n| u16::try_from(n).ok())
}

fn saved_metrics(network: Option<&dyn TableLike>) -> Option<u16> {
    network
        .and_then(|n| n.string("metrics"))
        .and_then(|address| {
            address
                .rsplit(':')
                .next()
                .and_then(|port| port.parse().ok())
        })
}

/// A random port in the dynamic range that nothing in `taken` uses and `is_free` accepts, if one turns up.
fn random_free_port(taken: &HashSet<u16>, is_free: &dyn Fn(u16) -> bool) -> u16 {
    let mut candidates = std::iter::repeat_with(random_dynamic_port)
        .filter(|port| !taken.contains(port))
        .take(ATTEMPTS);
    let first = candidates.next().unwrap_or(*DYNAMIC.start());
    std::iter::once(first)
        .chain(candidates)
        .find(|port| is_free(*port))
        .unwrap_or(first)
}

fn random_dynamic_port() -> u16 {
    let span = u64::from(DYNAMIC.end() - DYNAMIC.start()) + 1;
    let offset = RandomState::new().hash_one(std::time::Instant::now()) % span;
    DYNAMIC.start() + offset as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml_edit::DocumentMut;

    fn network(text: &str) -> DocumentMut {
        text.parse().unwrap()
    }

    #[test]
    fn new_ports_are_distinct_and_dynamic() {
        let ports = NetworkPorts::new(6188, None, &|_| true);
        let all = [ports.api, ports.reload, ports.metrics];
        assert!(all.iter().all(|port| DYNAMIC.contains(port)));
        assert_eq!(HashSet::from(all).len(), 3);
    }

    #[test]
    fn taken_ports_are_skipped() {
        let blocked = HashSet::from([50000_u16, 50001]);
        let ports = NetworkPorts::new(6188, None, &|port| {
            !blocked.contains(&port) && port % 2 == 0
        });
        for port in [ports.api, ports.reload, ports.metrics] {
            assert!(!blocked.contains(&port) && port % 2 == 0);
        }
    }

    #[test]
    fn rechecking_moves_a_port_another_app_holds() {
        let saved = network("api_port = 50000\nreload_port = 50001\nmetrics = \"127.0.0.1:50002\"");
        let ports = NetworkPorts::rechecked(6188, Some(saved.as_table()), &|port| port != 50000);
        assert_ne!(ports.api, 50000);
        assert!(DYNAMIC.contains(&ports.api));
        assert_eq!([ports.reload, ports.metrics], [50001, 50002]);
    }

    #[test]
    fn rechecking_moves_the_old_defaults() {
        let saved = network("api_port = 3000\nreload_port = 3001\nmetrics = \"127.0.0.1:6189\"");
        let ports = NetworkPorts::rechecked(6188, Some(saved.as_table()), &|_| true);
        assert!(DYNAMIC.contains(&ports.api) && DYNAMIC.contains(&ports.reload));
        assert_eq!(ports.metrics, 6189);
    }

    #[test]
    fn saving_keeps_ports_even_while_they_look_taken() {
        let saved = network("api_port = 3000\nreload_port = 3001\nmetrics = \"127.0.0.1:6189\"");
        let ports = NetworkPorts::new(6188, Some(saved.as_table()), &|_| false);
        assert_eq!([ports.api, ports.reload, ports.metrics], [3000, 3001, 6189]);
    }
}
