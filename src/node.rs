use std::collections::HashMap;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};

use log::*;

use crate::configuration::{PublicKeyWithTime, PublicPeer, StaticConfiguration};
use crate::crypt_udp::{AddressedTo, AdvertisementPacket, LocalContactPacket, RouteDatabasePacket};
use crate::event::Event;
use crate::routedb::{RouteDBManager, RouteInfo};
use crate::wg_dev::map_to_ipv6;

pub trait Node {
    fn routedb_manager(&self) -> Option<&RouteDBManager> {
        None
    }
    fn routedb_manager_mut(&mut self) -> Option<&mut RouteDBManager> {
        None
    }
    fn process_route_database(&mut self, req: RouteDatabasePacket) -> Option<Vec<Event>> {
        self.routedb_manager_mut()
            .map(|db| db.process_route_database(req))
    }
    fn local_admin_port(&self) -> u16;
    fn is_reachable(&self) -> bool {
        false
    }
    fn is_distant_node(&self) -> bool {
        false
    }
    fn get_gateway(&self) -> Option<Ipv4Addr> {
        None
    }
    fn set_gateway(&mut self, _gateway: Option<Ipv4Addr>) {}
    fn get_gateway_for(&mut self) -> Option<&mut HashSet<Ipv4Addr>> {
        None
    }
    fn clear_gateway_for(&mut self) {
        if let Some(gf) = self.get_gateway_for() {
            gf.clear();
        }
    }
    fn add_gateway_for(&mut self, node: Ipv4Addr) {
        if let Some(gf) = self.get_gateway_for() {
            gf.insert(node);
        }
    }

    fn get_route_info(&self) -> Option<RouteInfo> {
        // let ri = RouteInfo {
        //     to: *wg_ip,
        //     local_admin_port: node.local_admin_port(),
        //     hop_cnt: 0,
        //     gateway: node.via_gateway(),
        // };
        None
    }
    fn visible_wg_endpoint(&self) -> Option<SocketAddr> {
        None
    }
    fn process_every_second(&mut self, now: u64, static_config: &StaticConfiguration)
        -> Vec<Event>;
    fn ok_to_delete_without_route(&self, _now: u64) -> bool {
        false
    }
    fn peer_wireguard_configuration(&self) -> Option<Vec<String>>;
    fn analyze_advertisement(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>);
    fn update_from_wireguard_configuration(
        &mut self,
        pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    );
    fn process_local_contact(&mut self, _local: LocalContactPacket) {
        warn!("process_local_contact: unexpected for StaticPeer and DynamicPeer");
    }
}

#[derive(Debug)]
pub struct StaticPeer {
    static_peer: PublicPeer,
    public_key: Option<PublicKeyWithTime>,
    gateway_for: HashSet<Ipv4Addr>,
    is_alive: bool,
    lastseen: u64,
    send_advertisement_seconds_count_down: usize,
    routedb_manager: RouteDBManager,
}
impl StaticPeer {
    pub fn from_public_peer(peer: &PublicPeer) -> Box<dyn Node> {
        Box::new(StaticPeer {
            static_peer: (*peer).clone(),
            public_key: None,
            gateway_for: HashSet::new(),
            is_alive: false,
            lastseen: 0,
            send_advertisement_seconds_count_down: 0,
            routedb_manager: RouteDBManager::default(),
        })
    }
}
impl Node for StaticPeer {
    fn routedb_manager(&self) -> Option<&RouteDBManager> {
        Some(&self.routedb_manager)
    }
    fn routedb_manager_mut(&mut self) -> Option<&mut RouteDBManager> {
        Some(&mut self.routedb_manager)
    }
    fn get_gateway_for(&mut self) -> Option<&mut HashSet<Ipv4Addr>> {
        Some(&mut self.gateway_for)
    }
    fn local_admin_port(&self) -> u16 {
        self.static_peer.admin_port
    }
    fn peer_wireguard_configuration(&self) -> Option<Vec<String>> {
        // Not considered here is, if the StaticPeer is not directly reachable.
        self.public_key.as_ref().map(|public_key| {
            let mut lines = vec![];
            let wg_ip = self.static_peer.wg_ip;
            let wg_ipv6 = map_to_ipv6(&wg_ip);
            lines.push(format!("PublicKey = {}", &public_key.key));
            lines.push(format!("AllowedIPs = {}/32", wg_ip));
            lines.push(format!("AllowedIPs = {}/128", wg_ipv6));
            for ip in self.gateway_for.iter() {
                lines.push(format!("AllowedIPs = {}/32", ip));
            }
            lines.push(format!("EndPoint = {}", self.static_peer.endpoint));
            lines
        })
    }
    fn is_reachable(&self) -> bool {
        self.is_alive
    }
    fn process_every_second(
        &mut self,
        now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];
        if self.is_alive && now - self.lastseen > 240 {
            // seems to be dead
            self.is_alive = false;
            info!(target: &self.static_peer.wg_ip.to_string(),"static peer is not alive");
        }

        if self.is_alive {
            // If StaticPeer is alive, then send all communications via the tunnel.
            // Not considered here is, if the StaticPeer is not directly reachable.
            if self.send_advertisement_seconds_count_down == 0 {
                self.send_advertisement_seconds_count_down = 60;

                let destination =
                    SocketAddrV4::new(self.static_peer.wg_ip, self.static_peer.admin_port);

                let destination = SocketAddr::V4(destination);

                // Every 60s send an advertisement to the wireguard address
                events.push(Event::SendAdvertisement {
                    addressed_to: AddressedTo::WireguardAddress,
                    to: destination,
                    wg_ip: self.static_peer.wg_ip,
                });
            }
            if now % 10 == 0 && self.routedb_manager.is_outdated() {
                // if the local copy is not matching with latest info from StaticPeer,
                // then request an update.
                let destination =
                    SocketAddrV4::new(self.static_peer.wg_ip, self.static_peer.admin_port);
                events.push(Event::SendRouteDatabaseRequest { to: destination });
            }
        } else {
            // If static peer is not alive, send every 60s an advertisement
            // to the known endpoint
            if self.send_advertisement_seconds_count_down == 0 {
                self.send_advertisement_seconds_count_down = 60;

                // Resolve here the hostname (if not an IP) to make it work for dyndns hosts
                match self.static_peer.endpoint.to_socket_addrs() {
                    Ok(endpoints) => {
                        trace!("ENDPOINTS: {:#?}", endpoints);
                        for sa in endpoints {
                            // send to the endpoint with the admin_port as target
                            let destination = SocketAddr::new(sa.ip(), self.static_peer.admin_port);
                            events.push(Event::SendAdvertisement {
                                addressed_to: AddressedTo::StaticAddress,
                                to: destination,
                                wg_ip: self.static_peer.wg_ip,
                            });
                        }
                    }
                    Err(e) => {
                        // An error here is not dramatic, perhaps DNS is not reachable in the
                        // moment. Just push out a warning and that's it
                        warn!(
                            "Cannot get endpoint ip(s) for {}: {:?}",
                            self.static_peer.endpoint, e
                        );
                    }
                }
            }
        }
        self.send_advertisement_seconds_count_down -= 1;

        events
    }
    fn analyze_advertisement(
        &mut self,
        now: u64,
        _static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>) {
        let mut events = vec![];

        // advertisement has been received. Store the advertised routedb_version
        self.routedb_manager
            .latest_version(advertisement.routedb_version);

        // btw the StaticPeer is actually alive
        self.is_alive = true;
        self.lastseen = now;

        let mut reply_advertisement = false;
        if self.public_key.is_some() {
            // Check if public_key including creation time is same
            if self.public_key.as_ref().unwrap().key != advertisement.public_key.key {
                // Different public_key. Accept the one from advertisement only, if not older
                if self.public_key.as_ref().unwrap().priv_key_creation_time
                    <= advertisement.public_key.priv_key_creation_time
                {
                    self.public_key = Some(advertisement.public_key);
                    self.routedb_manager.invalidate();

                    info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
                    events.push(Event::UpdateWireguardConfiguration);

                    // As this peer is new, send an advertisement
                    reply_advertisement = true;
                } else {
                    warn!(target: "advertisement", "Received advertisement with old public key => Reject");
                }
            } else {
                // identical public key. So check, if the advertisement has been sent via the
                // tunnel
                if src_addr.ip() != self.static_peer.wg_ip {
                    // No. So the StaticPeer cannot send directly, then return the advertisement,
                    // if this is not already a reply
                    if !advertisement.addressed_to.is_reply() {
                        info!(target: "advertisement", "Advertisement from existing peer {} at public ip", src_addr);
                        reply_advertisement = true;
                    }
                } else {
                    // has come via tunnel, so nothing else to do
                    info!(target: "advertisement", "Advertisement from existing peer {}", src_addr);
                }
            }
        } else {
            // first time to see the StaticPeer
            self.public_key = Some(advertisement.public_key);
            self.routedb_manager.invalidate();
            events.push(Event::UpdateWireguardConfiguration);
            events.push(Event::UpdateRoutes);
            // As this peer is new, send an advertisement
            reply_advertisement = true;
        }
        if reply_advertisement {
            events.push(Event::SendAdvertisement {
                addressed_to: advertisement.addressed_to.reply(),
                to: src_addr,
                wg_ip: self.static_peer.wg_ip,
            });
        }
        (None, events)
    }
    fn update_from_wireguard_configuration(
        &mut self,
        _pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    ) {
        // Nothing to be done here for the moment
    }
}

#[derive(Debug)]
pub enum ConnectionType {
    Static {
        endpoint: SocketAddr,
        admin_endpoint: SocketAddr,
    },
    Local {
        endpoint: SocketAddr,
        admin_endpoint: SocketAddr,
    },
    Dynamic {
        endpoint: Option<SocketAddr>,
    },
    Passive,
}
impl ConnectionType {
    fn endpoint(&self) -> Option<SocketAddr> {
        match self {
            ConnectionType::Passive => None,
            ConnectionType::Static { endpoint, .. } => Some(*endpoint),
            ConnectionType::Local { endpoint, .. } => Some(*endpoint),
            ConnectionType::Dynamic { endpoint } => *endpoint,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            ConnectionType::Passive => "passive",
            ConnectionType::Static { .. } => "static",
            ConnectionType::Local { .. } => "local",
            ConnectionType::Dynamic { .. } => "dynamic",
        }
    }
}

#[derive(Debug)]
pub struct DynamicPeer {
    pub public_key: PublicKeyWithTime,
    pub local_wg_port: u16,
    pub local_admin_port: u16,
    pub wg_ip: Ipv4Addr,
    pub name: String,
    pub connection: ConnectionType,
    pub local_reachable_wg_endpoint: Option<SocketAddr>,
    pub local_reachable_admin_endpoint: Option<SocketAddr>,
    pub dp_visible_wg_endpoint: Option<SocketAddr>,
    pub gateway_for: HashSet<Ipv4Addr>,
    pub admin_port: u16,
    pub lastseen: u64,
    routedb_manager: RouteDBManager,
}
impl DynamicPeer {
    pub fn from_advertisement(
        now: u64,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> Option<Self> {
        let connection: ConnectionType;
        let mut local_reachable_admin_endpoint = None;
        let mut local_reachable_wg_endpoint = None;
        let mut dp_visible_wg_endpoint = None;

        use AddressedTo::*;
        match &advertisement.addressed_to {
            StaticAddress => {
                // seems the peer thinks, I am a static node.
                if static_config.is_static {
                    info!("StaticAddress: needs more work");
                    connection = ConnectionType::Passive;
                } else {
                    warn!("StaticAddress: needs more work");
                    return None;
                }
            }
            ReplyFromStaticAddress => {
                // The peer is a static node. So the defined endpoint can be used.
                if let Some(static_peer) = static_config.peers.get(&advertisement.wg_ip) {
                    let endpoint = SocketAddr::new(src_addr.ip(), static_peer.wg_port);
                    connection = ConnectionType::Static {
                        endpoint,
                        admin_endpoint: src_addr,
                    };
                } else {
                    warn!("ReplyFromStaticAddress: reply from static address, but peer is not a static peer");
                    return None;
                }

                // But the static peer gives us info about our visible address.
                // If this is not a "local" address, then this is after a firewall
                // and can be used for NAT traversal for other node. So store this info
                if let Some(visible_endpoint) = advertisement.your_visible_wg_endpoint.as_ref() {
                    let mut is_local = false;

                    for ip in static_config.ip_list.iter() {
                        if *ip == visible_endpoint.ip() {
                            is_local = true;
                            break;
                        }
                    }
                    if !is_local {
                        dp_visible_wg_endpoint = Some(*visible_endpoint);
                    }
                }
            }
            LocalAddress | ReplyFromLocalAddress => {
                // The peer and myself are talking in the same subnet.
                // So the peer's endpoint can be determined.
                let endpoint = SocketAddr::new(src_addr.ip(), advertisement.local_wg_port);
                connection = ConnectionType::Local {
                    endpoint,
                    admin_endpoint: src_addr,
                };
                local_reachable_wg_endpoint = Some(endpoint);
                local_reachable_admin_endpoint = Some(src_addr);
            }
            WireguardV6Address | ReplyFromWireguardV6Address => {
                // Apparently NAT traversal was successful and we are in contact.
                // dp_visible_wg_endpoint = advertisement.my_visible_wg_endpoint;
                // As soon as the peer is registered as DynamicPeer, the route will be adjusted
                // accordingly
                connection = ConnectionType::Dynamic {
                    endpoint: advertisement.your_visible_wg_endpoint,
                }
            }
            WireguardAddress | ReplyFromWireguardAddress => {
                warn!(target: &advertisement.wg_ip.to_string(), "unexpected advertisement to wireguard address for new dynamic peer => ignore");
                return None;
            }
        }
        let mut routedb_manager = RouteDBManager::default();
        routedb_manager.latest_version(advertisement.routedb_version);
        Some(DynamicPeer {
            wg_ip: advertisement.wg_ip,
            local_admin_port: advertisement.local_admin_port,
            local_wg_port: advertisement.local_wg_port,
            public_key: advertisement.public_key.clone(),
            name: advertisement.name,
            connection,
            local_reachable_admin_endpoint,
            local_reachable_wg_endpoint,
            dp_visible_wg_endpoint,
            gateway_for: HashSet::new(),
            admin_port: src_addr.port(),
            lastseen: now,
            routedb_manager,
        })
    }
}
impl Node for DynamicPeer {
    fn routedb_manager(&self) -> Option<&RouteDBManager> {
        Some(&self.routedb_manager)
    }
    fn routedb_manager_mut(&mut self) -> Option<&mut RouteDBManager> {
        Some(&mut self.routedb_manager)
    }
    fn get_gateway_for(&mut self) -> Option<&mut HashSet<Ipv4Addr>> {
        Some(&mut self.gateway_for)
    }
    fn visible_wg_endpoint(&self) -> Option<SocketAddr> {
        self.dp_visible_wg_endpoint
    }
    fn local_admin_port(&self) -> u16 {
        self.local_admin_port
    }
    fn is_reachable(&self) -> bool {
        true
    }
    fn peer_wireguard_configuration(&self) -> Option<Vec<String>> {
        let mut lines = vec![];
        lines.push(format!("PublicKey = {}", &self.public_key.key));
        lines.push(format!("AllowedIPs = {}/32", self.wg_ip));
        lines.push(format!("AllowedIPs = {}/128", map_to_ipv6(&self.wg_ip)));
        for ip in self.gateway_for.iter() {
            lines.push(format!("AllowedIPs = {}/32", ip));
        }
        if let Some(endpoint) = self.connection.endpoint() {
            debug!(target: "configuration", "peer {} uses {} endpoint {}", self.wg_ip, self.connection.as_str(), endpoint);
            debug!(target: &self.wg_ip.to_string(), "use {} endpoint {}", self.connection.as_str(), endpoint);
            lines.push(format!("EndPoint = {}", endpoint));
        } else {
            debug!(target: "configuration", "dynamic peer {} without endpoint", self.wg_ip);
            debug!(target: &self.wg_ip.to_string(), "is dynamic peer without endpoint");
        }
        Some(lines)
    }
    fn process_every_second(
        &mut self,
        now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        let dt = now - self.lastseen;
        if dt % 30 == 29 {
            // Request routedb update, if outdated
            if self.routedb_manager.is_outdated() {
                let destination = SocketAddrV4::new(self.wg_ip, self.admin_port);
                events.push(Event::SendRouteDatabaseRequest { to: destination });
            }

            // Pings are sent out only via the wireguard interface.
            let destination = SocketAddr::V4(SocketAddrV4::new(self.wg_ip, self.admin_port));
            events.push(Event::SendAdvertisement {
                addressed_to: AddressedTo::WireguardAddress,
                to: destination,
                wg_ip: self.wg_ip,
            });
        }
        events
    }
    fn ok_to_delete_without_route(&self, now: u64) -> bool {
        let dt = now - self.lastseen;
        dt > 120
    }
    fn analyze_advertisement(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>) {
        let mut events = vec![];
        self.lastseen = now;

        // Check if public_key including creation time is same
        if self.public_key != advertisement.public_key {
            // Different public_key. Accept the one from advertisement only, if not older
            if self.public_key.priv_key_creation_time
                <= advertisement.public_key.priv_key_creation_time
            {
                info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);

                // As this peer is new, send an advertisement
                info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
                events.push(Event::SendAdvertisement {
                    addressed_to: advertisement.addressed_to.reply(),
                    to: src_addr,
                    wg_ip: advertisement.wg_ip,
                });

                // new public key to be added to wireguard - eventually still without endpoint
                events.push(Event::UpdateWireguardConfiguration);

                // and replace myself together with route update
                events.push(Event::UpdateRoutes);

                if let Some(dp) =
                    DynamicPeer::from_advertisement(now, static_config, advertisement, src_addr)
                {
                    return (Some(Box::new(dp)), events);
                } else {
                    return (None, vec![]);
                }
            } else {
                warn!(target: "advertisement", "Received advertisement with old public key => Reject");
            }
        } else {
            info!(target: "advertisement", "Advertisement from existing peer {}", src_addr);

            self.routedb_manager
                .latest_version(advertisement.routedb_version);

            use crate::crypt_udp::AddressedTo::*;
            match advertisement.addressed_to {
                StaticAddress => {
                    // For whatever reason the peer sends not via the tunnel.
                    // Was the connection dropped or endpoint is not correct ?
                    // or a late package addressed to distant node ?
                    warn!(target: "advertisement", "has not been sent via tunnel");
                    if advertisement.your_visible_wg_endpoint.is_some() {
                        events.push(Event::UpdateWireguardConfiguration);
                        self.dp_visible_wg_endpoint = advertisement.my_visible_wg_endpoint;
                    }
                    events.push(Event::SendAdvertisement {
                        addressed_to: advertisement.addressed_to.reply(),
                        to: src_addr,
                        wg_ip: self.wg_ip,
                    });
                }
                ReplyFromStaticAddress => {
                    warn!(target: "advertisement", "reply has not been sent via tunnel");
                    if self.dp_visible_wg_endpoint.is_none()
                        && advertisement.your_visible_wg_endpoint.is_some()
                    {
                        events.push(Event::UpdateWireguardConfiguration);
                        self.dp_visible_wg_endpoint = advertisement.my_visible_wg_endpoint;
                    }
                }
                LocalAddress => {
                    // For whatever reason the peer sends not via the tunnel.
                    // Was the connection dropped or endpoint is not correct ?
                    // or a late package addressed to distant node ?
                    warn!(target: "advertisement", "has not been sent via tunnel");
                    events.push(Event::SendAdvertisement {
                        addressed_to: advertisement.addressed_to.reply(),
                        to: src_addr,
                        wg_ip: self.wg_ip,
                    });
                }
                ReplyFromLocalAddress => {
                    warn!(target: "advertisement", "reply has not been sent via tunnel");
                }
                WireguardAddress
                | WireguardV6Address
                | ReplyFromWireguardAddress
                | ReplyFromWireguardV6Address => {
                    // tunnel is ok. So check for visible wg endpoints
                    if self.dp_visible_wg_endpoint.is_none() {
                        events.push(Event::ReadWireguardConfiguration);
                    }
                }
            }
        }
        (None, vec![])
    }
    fn update_from_wireguard_configuration(
        &mut self,
        pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    ) {
        if let Some(endpoint) = pubkey_to_endpoint.remove(&self.public_key.key) {
            self.dp_visible_wg_endpoint = Some(endpoint);
        }
    }
}

#[derive(Debug)]
pub struct DistantNode {
    pub wg_ip: Ipv4Addr,
    admin_port: u16,
    //hop_cnt: usize,
    //gateway: Option<Ipv4Addr>,
    pub public_key: Option<PublicKeyWithTime>,
    known_in_s: usize,
    local_ip_list: Option<Vec<IpAddr>>,
    local_admin_port: Option<u16>,
    send_count: usize,
    can_send_to_visible_endpoint: bool,
    pub visible_endpoint: Option<SocketAddr>,
    gateway: Option<Ipv4Addr>,
}
impl DistantNode {
    pub fn from(ri: &RouteInfo) -> Self {
        DistantNode {
            wg_ip: ri.to,
            admin_port: ri.local_admin_port,
            //hop_cnt: ri.hop_cnt,
            //gateway: ri.gateway,
            public_key: None,
            known_in_s: 0,
            local_ip_list: None,
            local_admin_port: None,
            send_count: 0,
            can_send_to_visible_endpoint: false,
            visible_endpoint: None,
            gateway: None,
        }
    }
}
impl Node for DistantNode {
    fn process_local_contact(&mut self, local: LocalContactPacket) {
        debug!(target: &self.wg_ip.to_string(), "Received local contact packet");
        self.send_count = 0;
        self.local_ip_list = Some(local.local_ip_list);
        self.local_admin_port = Some(local.local_admin_port);
        self.visible_endpoint = local.my_visible_wg_endpoint;
        self.public_key = Some(local.public_key);
    }
    fn peer_wireguard_configuration(&self) -> Option<Vec<String>> {
        self.public_key.as_ref().map(
            |public_key| {
            let mut lines = vec![];
            lines.push(format!("PublicKey = {}", &public_key.key));
            lines.push(format!("AllowedIPs = {}/128", map_to_ipv6(&self.wg_ip)));
            if let Some(endpoint) = self.visible_endpoint.as_ref() {
                warn!("peer sends eventually local address as visible endpoint");
                debug!(target: "configuration", "node {} uses visible (NAT) endpoint {}", self.wg_ip, endpoint);
                debug!(target: &self.wg_ip.to_string(), "use visible (NAT) endpoint {}", endpoint);
                lines.push(format!("EndPoint = {}", endpoint));
            }
            lines
        })
    }
    fn process_every_second(
        &mut self,
        now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        let pk_available = if self.public_key.is_some() {
            ", public key available"
        } else {
            ""
        };
        self.known_in_s += 1;

        if self.local_ip_list.is_none()
            || self.public_key.is_none()
            || self.visible_endpoint.is_none()
        {
            // have no data received or is not complete, so ask again
            if self.known_in_s % 60 == 0 || self.known_in_s < 5 {
                // Send request for local contact
                trace!(target: "nodes", "Alive node: {:?} for {} s {}", self.wg_ip, self.known_in_s, pk_available);
                let destination = SocketAddrV4::new(self.wg_ip, self.admin_port);
                events.push(Event::SendLocalContactRequest { to: destination });
            }
        }
        if self.send_count < 10 {
            // Try to reach local ip
            if let Some(ip_list) = self.local_ip_list.as_ref() {
                if let Some(admin_port) = self.local_admin_port.as_ref() {
                    self.send_count += 1;
                    info!(target: &self.wg_ip.to_string(), "try to reach distant node via local subnet {}/10",self.send_count);
                    for ip in ip_list.iter() {
                        if let IpAddr::V4(ipv4) = ip {
                            if *ipv4 == self.wg_ip {
                                continue;
                            }
                        }
                        events.push(Event::SendAdvertisement {
                            addressed_to: AddressedTo::LocalAddress,
                            to: SocketAddr::new(*ip, *admin_port),
                            wg_ip: self.wg_ip,
                        });
                    }
                }
            }
        }
        let can_send = self.public_key.is_some() && self.visible_endpoint.is_some();

        if can_send {
            if !self.can_send_to_visible_endpoint {
                self.can_send_to_visible_endpoint = true;
                events.push(Event::UpdateWireguardConfiguration);
            }

            if now % 60 < 5 {
                // TODO: Try to reach visible endpoint via wg ipv6
                info!(target: &self.wg_ip.to_string(), "try to reach distant node via NAT traversal");
                let wg_ipv6 = map_to_ipv6(&self.wg_ip);
                let destination = SocketAddr::V6(SocketAddrV6::new(wg_ipv6, self.admin_port, 0, 0));
                events.push(Event::SendAdvertisement {
                    addressed_to: AddressedTo::WireguardV6Address,
                    to: destination,
                    wg_ip: self.wg_ip,
                });
            }
        }

        events
    }
    fn ok_to_delete_without_route(&self, _now: u64) -> bool {
        // only delete, if dropped from routing table
        false
    }
    fn analyze_advertisement(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>) {
        let mut events = vec![];

        let reply = advertisement.addressed_to.reply();
        if let Some(dp) =
            DynamicPeer::from_advertisement(now, static_config, advertisement, src_addr)
        {
            // As this peer is new, send an advertisement
            info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
            events.push(Event::SendAdvertisement {
                addressed_to: reply,
                to: src_addr,
                wg_ip: dp.wg_ip,
            });
            events.push(Event::UpdateWireguardConfiguration);

            // if still not known, then ask wireguard
            if dp.dp_visible_wg_endpoint.is_none() {
                events.push(Event::ReadWireguardConfiguration);
            }

            (Some(Box::new(dp)), events)
        } else {
            (None, events)
        }
    }
    fn update_from_wireguard_configuration(
        &mut self,
        pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    ) {
        if let Some(public_key) = self.public_key.as_ref() {
            if let Some(endpoint) = pubkey_to_endpoint.remove(&public_key.key) {
                let mut is_local = false;

                for ip in self.local_ip_list.as_ref().unwrap().iter() {
                    if *ip == endpoint.ip() {
                        is_local = true;
                        break;
                    }
                }
                if !is_local {
                    self.visible_endpoint = Some(endpoint);
                }
            }
        }
    }
    fn local_admin_port(&self) -> u16 {
        self.admin_port
    }
    fn is_distant_node(&self) -> bool {
        true
    }
    fn get_gateway(&self) -> Option<Ipv4Addr> {
        self.gateway
    }
    fn set_gateway(&mut self, gateway: Option<Ipv4Addr>) {
        self.gateway = gateway;
    }
}
