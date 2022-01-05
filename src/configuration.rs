use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Instant;

use log::*;
use serde::{Deserialize, Serialize};

use crate::manager::*;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct PublicKeyWithTime {
    pub key: String, // base64 encoded
    pub priv_key_creation_time: u64,
}

pub struct PublicPeer {
    pub public_ip: IpAddr,
    pub comm_port: u16,
    pub admin_port: u16,
    pub wg_ip: Ipv4Addr,
}

#[derive(Default)]
pub struct StaticConfigurationBuilder {
    name: Option<String>,
    ip_list: Option<Vec<IpAddr>>,
    wg_ip: Option<Ipv4Addr>,
    wg_name: Option<String>,
    shared_key: Option<Vec<u8>>,
    my_private_key: Option<String>,
    my_public_key: Option<PublicKeyWithTime>,
    peers: Vec<PublicPeer>,
}
impl StaticConfigurationBuilder {
    pub fn new() -> Self {
        StaticConfigurationBuilder::default()
    }
    pub fn name<T: Into<String>>(mut self, name: T) -> Self {
        self.name = Some(name.into());
        self
    }
    pub fn ip_list(mut self, ip_list: Vec<IpAddr>) -> Self {
        self.ip_list = Some(ip_list);
        self
    }
    pub fn wg_ip<T: Into<Ipv4Addr>>(mut self, wg_ip: T) -> Self {
        self.wg_ip = Some(wg_ip.into());
        self
    }
    pub fn wg_name<T: Into<String>>(mut self, wg_name: T) -> Self {
        self.wg_name = Some(wg_name.into());
        self
    }
    pub fn shared_key(mut self, shared_key: Vec<u8>) -> Self {
        self.shared_key = Some(shared_key);
        self
    }
    pub fn my_private_key<T: Into<String>>(mut self, private_key: T) -> Self {
        self.my_private_key = Some(private_key.into());
        self
    }
    pub fn my_public_key(mut self, public_key: PublicKeyWithTime) -> Self {
        self.my_public_key = Some(public_key);
        self
    }
    pub fn peers(mut self, peers: Vec<PublicPeer>) -> Self {
        self.peers = peers;
        self
    }
    pub fn build(self) -> StaticConfiguration {
        let mut myself_as_peer: Option<usize> = None;
        for (i, peer) in self.peers.iter().enumerate() {
            if &peer.wg_ip == self.wg_ip.as_ref().unwrap() {
                debug!("FOUND myself as listener");
                myself_as_peer = Some(i);
                break;
            }
        }

        let peer_cnt = self.peers.len();
        StaticConfiguration {
            name: self.name.unwrap(),
            ip_list: self.ip_list.unwrap(),
            wg_ip: self.wg_ip.unwrap(),
            wg_name: self.wg_name.unwrap(),
            myself_as_peer,
            shared_key: self.shared_key.unwrap(),
            my_private_key: self.my_private_key.unwrap(),
            my_public_key: self.my_public_key.unwrap(),
            peers: self.peers,
            peer_cnt,
        }
    }
}

pub struct StaticConfiguration {
    pub name: String,
    pub ip_list: Vec<IpAddr>,
    pub wg_ip: Ipv4Addr,
    pub wg_name: String,
    myself_as_peer: Option<usize>,
    pub shared_key: Vec<u8>,
    pub my_private_key: String,
    pub my_public_key: PublicKeyWithTime,
    pub peers: Vec<PublicPeer>,
    pub peer_cnt: usize,
}

impl StaticConfiguration {
    pub fn builder() -> StaticConfigurationBuilder {
        StaticConfigurationBuilder::new()
    }
    pub fn is_listener(&self) -> bool {
        self.myself_as_peer.is_some()
    }
    pub fn as_conf_as_peer(
        &self,
        dynamic_peers: Option<&DynamicPeerList>,
        manager: &NetworkManager,
    ) -> String {
        let mut lines: Vec<String> = vec![];
        lines.push("[Interface]".to_string());
        lines.push(format!("PrivateKey = {}", self.my_private_key));
        if let Some(myself) = self.myself_as_peer {
            let peer = &self.peers[myself];
            lines.push(format!("ListenPort = {}", peer.comm_port));
        }
        lines.push("".to_string());

        if let Some(peers) = dynamic_peers {
            for peer in peers.peer.values() {
                lines.push("[Peer]".to_string());
                lines.push(format!("PublicKey = {}", &peer.public_key.key));
                lines.push(format!("AllowedIPs = {}/32", peer.wg_ip));
                let ips = manager.get_ips_for_peer(peer.wg_ip);
                for ip in ips {
                    lines.push(format!("AllowedIPs = {}/32", ip));
                }
                if let Some(endpoint) = peer.endpoint.as_ref() {
                    lines.push(format!("EndPoint = {}", endpoint));
                }
                lines.push("".to_string());
            }
        }

        lines.join("\n")
    }
    pub fn my_admin_port(&self) -> Option<u16> {
        self.myself_as_peer.map(|i| self.peers[i].admin_port)
    }
    pub fn admin_port(&self, peer_index: usize) -> u16 {
        self.peers[peer_index].admin_port
    }
}

#[derive(Debug)]
pub struct DynamicPeer {
    pub public_key: PublicKeyWithTime,
    pub local_ip_list: Vec<IpAddr>,
    pub wg_ip: Ipv4Addr,
    pub name: String,
    pub endpoint: Option<SocketAddr>,
    pub admin_port: u16,
    pub lastseen: Instant,
}

#[derive(Default)]
pub struct DynamicPeerList {
    pub peer: HashMap<Ipv4Addr, DynamicPeer>,
    pub fifo_dead: Vec<Ipv4Addr>,
    pub fifo_ping: Vec<Ipv4Addr>,
}
impl DynamicPeerList {
    pub fn analyze_advertisement(
        &mut self,
        from_advertisement: &UdpPacket,
        admin_port: u16,
    ) -> Option<Ipv4Addr> {
        use UdpPacket::*;
        match from_advertisement {
            RouteDatabaseRequest { .. } => None,
            RouteDatabase { .. } => None,
            Advertisement {
                public_key,
                local_ip_list,
                wg_ip,
                name,
                endpoint,
                routedb_version: _,
            } => {
                self.fifo_dead.push(*wg_ip);
                self.fifo_ping.push(*wg_ip);
                let lastseen = Instant::now();
                if self
                    .peer
                    .insert(
                        *wg_ip,
                        DynamicPeer {
                            wg_ip: *wg_ip,
                            local_ip_list: local_ip_list.clone(),
                            public_key: public_key.clone(),
                            name: name.to_string(),
                            endpoint: *endpoint,
                            admin_port,
                            lastseen,
                        },
                    )
                    .is_none()
                {
                    Some(*wg_ip)
                } else {
                    None
                }
            }
        }
    }
    pub fn check_timeouts(&mut self, limit: u64) -> HashSet<Ipv4Addr> {
        let mut dead_peers = HashSet::new();
        while let Some(wg_ip) = self.fifo_dead.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                if peer.lastseen.elapsed().as_secs() < limit {
                    break;
                }
                dead_peers.insert(**wg_ip);
            }
            self.fifo_dead.remove(0);
        }
        dead_peers
    }
    pub fn check_ping_timeouts(&mut self, limit: u64) -> HashSet<(Ipv4Addr, u16)> {
        let mut ping_peers = HashSet::new();
        while let Some(wg_ip) = self.fifo_ping.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                if peer.lastseen.elapsed().as_secs() < limit {
                    break;
                }
                ping_peers.insert((**wg_ip, peer.admin_port));
            }
            self.fifo_ping.remove(0);
        }
        ping_peers
    }
    pub fn remove_peer(&mut self, wg_ip: &Ipv4Addr) {
        self.peer.remove(wg_ip);
    }
    pub fn knows_peer(&mut self, wg_ip: &Ipv4Addr) -> bool {
        self.peer.contains_key(wg_ip)
    }
    pub fn output(&self) {
        for peer in self.peer.values() {
            info!("{:?}", peer);
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum UdpPacket {
    Advertisement {
        public_key: PublicKeyWithTime,
        local_ip_list: Vec<IpAddr>,
        wg_ip: Ipv4Addr,
        name: String,
        endpoint: Option<SocketAddr>,
        routedb_version: usize,
    },
    RouteDatabaseRequest {},
    RouteDatabase {
        sender: Ipv4Addr,
        routedb_version: usize,
        nr_entries: usize,
        known_routes: Vec<RouteInfo>,
    },
}
impl UdpPacket {
    pub fn advertisement_from_config(
        static_config: &StaticConfiguration,
        routedb_version: usize,
    ) -> Self {
        let endpoint = if static_config.is_listener() {
            let peer = &static_config.peers[static_config.myself_as_peer.unwrap()];
            Some(SocketAddr::new(peer.public_ip, peer.comm_port))
        } else {
            None
        };
        UdpPacket::Advertisement {
            public_key: static_config.my_public_key.clone(),
            local_ip_list: static_config.ip_list.clone(),
            wg_ip: static_config.wg_ip,
            name: static_config.name.clone(),
            endpoint,
            routedb_version,
        }
    }
    pub fn route_database_request() -> Self {
        UdpPacket::RouteDatabaseRequest {}
    }
    pub fn make_route_database(
        sender: Ipv4Addr,
        routedb_version: usize,
        nr_entries: usize,
        known_routes: Vec<&RouteInfo>,
    ) -> Self {
        UdpPacket::RouteDatabase {
            sender,
            routedb_version,
            nr_entries,
            known_routes: known_routes.into_iter().cloned().collect(),
        }
    }
}
impl fmt::Debug for UdpPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UdpPacket::Advertisement { .. } => f.debug_struct("Adverisement"),
            UdpPacket::RouteDatabaseRequest { .. } => f.debug_struct("RouteDatabaseRequest"),
            UdpPacket::RouteDatabase { .. } => f.debug_struct("RouteDatabase"),
        }
        .finish()
    }
}
