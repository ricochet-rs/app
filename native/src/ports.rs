use crate::config::section::TableRead;
use socket2::{Domain, Socket, Type};
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddrV4};
use toml_edit::TableLike;

/// The ports ricochet-server listens on, chosen so none collides with another app or with each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkPorts {
    pub proxy: u16,
    pub api: u16,
    pub reload: u16,
    pub metrics: u16,
}

impl NetworkPorts {
    /// Keeps the ports already saved in `network`, because ricochet holds them itself,
    /// and probes for a free one only where none is saved.
    pub fn new(proxy: u16, network: Option<&dyn TableLike>, is_free: &dyn Fn(u16) -> bool) -> Self {
        let mut taken = HashSet::from([proxy]);
        let saved = |key: &str| {
            network
                .and_then(|n| n.int(key))
                .and_then(|n| u16::try_from(n).ok())
        };
        let saved_metrics = network
            .and_then(|n| n.string("metrics"))
            .and_then(|address| {
                address
                    .rsplit(':')
                    .next()
                    .and_then(|port| port.parse().ok())
            });
        Self {
            proxy,
            api: claim(&mut taken, saved("api_port"), 3000, is_free),
            reload: claim(&mut taken, saved("reload_port"), 3001, is_free),
            metrics: claim(&mut taken, saved_metrics, 6189, is_free),
        }
    }

    /// Whether ricochet could bind `port`, probed with `SO_REUSEADDR` as ricochet binds,
    /// so connections ricochet just closed don't make its own port look taken.
    pub fn is_free(port: u16) -> bool {
        let Ok(socket) = Socket::new(Domain::IPV4, Type::STREAM, None) else {
            return false;
        };
        let _ = socket.set_reuse_address(true);
        socket
            .bind(&SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).into())
            .is_ok()
    }
}

fn claim(
    taken: &mut HashSet<u16>,
    saved: Option<u16>,
    preferring: u16,
    is_free: &dyn Fn(u16) -> bool,
) -> u16 {
    if let Some(saved) = saved
        && !taken.contains(&saved)
    {
        taken.insert(saved);
        return saved;
    }
    let port = (preferring..=preferring.saturating_add(100))
        .find(|port| !taken.contains(port) && is_free(*port))
        .unwrap_or(preferring);
    taken.insert(port);
    port
}
