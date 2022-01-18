use std::collections::HashMap;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs};

use log::*;

use crate::configuration::{PublicKeyWithTime, PublicPeer, StaticConfiguration};
use crate::crypt_udp::{AddressedTo, AdvertisementPacket, LocalContactPacket};
use crate::event::Event;
use crate::manager::{RouteInfo, PeerRouteDB};
use crate::wg_dev::map_to_ipv6;

#[derive(Default, Debug)]
pub struct RouteDBManager {
    routedb: Option<PeerRouteDB>,
    incoming_routedb: Option<PeerRouteDB>,
    latest_routedb_version: Option<usize>,
}
impl RouteDBManager {
    fn is_outdated(&self) -> bool {
        self.routedb.as_ref().map(|db| db.version) != self.latest_routedb_version
    }
    fn latest_version(&mut self, version: usize) {
        self.latest_routedb_version = Some(version);
    }
    fn invalidate(&mut self) {
        self.routedb = None;
        self.incoming_routedb = None;
        self.latest_routedb_version = None;
    }
}

pub trait Node {
    fn routedb_manager(&mut self) -> Option<&mut RouteDBManager> {
        None
    }
    fn local_admin_port(&self) -> u16;
    fn is_reachable(&self) -> bool {
        false
    }
    fn via_gateway(&self) -> Option<Ipv4Addr> {
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
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>);
    fn update_from_wireguard_configuration(
        &mut self,
        pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    );
    fn process_local_contact(&mut self, local: LocalContactPacket) {
        warn!("process_local_contact: not implemented");
    }
}

#[derive(Debug)]
pub struct StaticPeer {
    static_peer: PublicPeer,
    public_key: Option<PublicKeyWithTime>,
    gateway_for: HashSet<Ipv4Addr>,
    is_alive: bool,
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
            send_advertisement_seconds_count_down: 0,
            routedb_manager: RouteDBManager::default(),
        })
    }
}
impl Node for StaticPeer {
    fn routedb_manager(&mut self) -> Option<&mut RouteDBManager> {
        Some(&mut self.routedb_manager)
    }
    fn local_admin_port(&self) -> u16 {
        self.static_peer.admin_port
    }
    fn peer_wireguard_configuration(&self) -> Option<Vec<String>> {
        self.public_key.as_ref().map(|public_key| {
            let mut lines = vec![];
            lines.push(format!("PublicKey = {}", &public_key.key));
            lines.push(format!("AllowedIPs = {}/32", self.static_peer.wg_ip));
            lines.push(format!(
                "AllowedIPs = {}/128",
                map_to_ipv6(&self.static_peer.wg_ip)
            ));
            for ip in self.gateway_for.iter() {
                lines.push(format!("AllowedIPs = {}/32", ip));
            }
            lines.push(format!("EndPoint = {}", self.static_peer.endpoint));
            lines
        })
    }
    fn process_every_second(
        &mut self,
        _now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        if !self.is_alive {
            if self.send_advertisement_seconds_count_down == 0 {
                self.send_advertisement_seconds_count_down = 60;

                if self.routedb_manager.is_outdated() {
                    let destination =
                        SocketAddrV4::new(self.static_peer.wg_ip, self.static_peer.admin_port);
                    events.push(Event::SendRouteDatabaseRequest { to: destination });
                }

                // Resolve here to make it work for dyndns hosts
                match self.static_peer.endpoint.to_socket_addrs() {
                    Ok(endpoints) => {
                        trace!("ENDPOINTS: {:#?}", endpoints);
                        for sa in endpoints {
                            let destination = SocketAddr::new(sa.ip(), self.static_peer.admin_port);
                            events.push(Event::SendAdvertisement {
                                addressed_to: AddressedTo::StaticAddress,
                                to: destination,
                                wg_ip: self.static_peer.wg_ip,
                            });
                        }
                    }
                    Err(e) => {
                        // An error here is not dramatic. Just push out a warning and
                        // that's it
                        warn!(
                            "Cannot get endpoint ip(s) for {}: {:?}",
                            self.static_peer.endpoint, e
                        );
                    }
                }
            } else {
                self.send_advertisement_seconds_count_down -= 1;
            }
        }

        events
    }
    fn analyze_advertisement(
        &mut self,
        _static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>) {
        let mut events = vec![];

        self.routedb_manager.latest_version(advertisement.routedb_version);
        self.is_alive = true;
        let mut send_advertisement = false;

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
                    send_advertisement = true;
                } else {
                    warn!(target: "advertisement", "Received advertisement with old public key => Reject");
                }
            } else {
                if src_addr.ip() != self.static_peer.wg_ip {
                    info!(target: "advertisement", "Advertisement from existing peer {} at public ip", src_addr);
                    send_advertisement = true;
                } else {
                    info!(target: "advertisement", "Advertisement from existing peer {}", src_addr);
                }
            }
        } else {
            self.public_key = Some(advertisement.public_key);
            self.routedb_manager.invalidate();
            events.push(Event::UpdateWireguardConfiguration);
            events.push(Event::UpdateRoutes);
            // As this peer is new, send an advertisement
            send_advertisement = true;
        }
        if send_advertisement {
            events.push(Event::SendAdvertisement {
                addressed_to: advertisement.addressed_to,
                to: src_addr,
                wg_ip: self.static_peer.wg_ip,
            });
        }
        (None, events)
    }
    fn update_from_wireguard_configuration(
        &mut self,
        pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    ) {
    }
}

#[derive(Debug)]
pub struct DynamicPeer {
    pub public_key: PublicKeyWithTime,
    pub local_wg_port: u16,
    pub local_admin_port: u16,
    pub wg_ip: Ipv4Addr,
    pub name: String,
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
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> Self {
        let lastseen = crate::util::now();

        let mut local_reachable_admin_endpoint = None;
        let mut local_reachable_wg_endpoint = None;
        let mut dp_visible_wg_endpoint = None;

        use AddressedTo::*;
        match &advertisement.addressed_to {
            StaticAddress | ReplyFromStaticAddress => {}
            LocalAddress | ReplyFromLocalAddress => {
                local_reachable_wg_endpoint =
                    Some(SocketAddr::new(src_addr.ip(), advertisement.local_wg_port));
                local_reachable_admin_endpoint = Some(src_addr);
            }
            WireguardV6Address | ReplyFromWireguardV6Address => {
                dp_visible_wg_endpoint = advertisement.my_visible_wg_endpoint;
            }
            WireguardAddress | ReplyFromWireguardAddress => {
                if advertisement.your_visible_wg_endpoint.is_some() {
                    let mut is_local = false;

                    for ip in static_config.ip_list.iter() {
                        if *ip
                            == advertisement
                                .your_visible_wg_endpoint
                                .as_ref()
                                .unwrap()
                                .ip()
                        {
                            is_local = true;
                        }
                    }
                }
            }
        }
        let mut routedb_manager = RouteDBManager::default();
        routedb_manager.latest_version(advertisement.routedb_version);
        DynamicPeer {
            wg_ip: advertisement.wg_ip,
            local_admin_port: advertisement.local_admin_port,
            local_wg_port: advertisement.local_wg_port,
            public_key: advertisement.public_key.clone(),
            name: advertisement.name.to_string(),
            local_reachable_admin_endpoint,
            local_reachable_wg_endpoint,
            dp_visible_wg_endpoint,
            gateway_for: HashSet::new(),
            admin_port: src_addr.port(),
            lastseen,
            routedb_manager,
        }
    }
}
impl Node for DynamicPeer {
    fn routedb_manager(&mut self) -> Option<&mut RouteDBManager> {
        Some(&mut self.routedb_manager)
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
        lines.push(format!("AllowedIPs = {}/128", map_to_ipv6(&self.wg_ip)));
        for ip in self.gateway_for.iter() {
            lines.push(format!("AllowedIPs = {}/32", ip));
        }
        if let Some(endpoint) = self.local_reachable_wg_endpoint.as_ref() {
            debug!(target: "configuration", "peer {} uses local endpoint {}", self.wg_ip, endpoint);
            debug!(target: &self.wg_ip.to_string(), "use local endpoint {}", endpoint);
            lines.push(format!("EndPoint = {}", endpoint));
        } else if let Some(endpoint) = self.dp_visible_wg_endpoint.as_ref() {
            debug!(target: "configuration", "peer {} uses visible (NAT) endpoint {}", self.wg_ip, endpoint);
            debug!(target: &self.wg_ip.to_string(), "use visible (NAT) endpoint {}", endpoint);
            lines.push(format!("EndPoint = {}", endpoint));
        }
        Some(lines)
    }
    fn process_every_second(
        &mut self,
        now: u64,
        _static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        // Pings are sent out only via the wireguard interface.
        //
        let dt = now - self.lastseen;
        if dt % 30 == 29 {
            if self.routedb_manager.is_outdated() {
                let destination = SocketAddrV4::new(self.wg_ip, self.admin_port);
                events.push(Event::SendRouteDatabaseRequest { to: destination });
            }

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
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>) {
        let mut events = vec![];

        let latest_routedb_version = advertisement.routedb_version;

        // Check if public_key including creation time is same
        if self.public_key != advertisement.public_key {
            // Different public_key. Accept the one from advertisement only, if not older
            if self.public_key.priv_key_creation_time
                <= advertisement.public_key.priv_key_creation_time
            {
                info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
                self.routedb_manager.invalidate();

                events.push(Event::UpdateWireguardConfiguration);

                // As this peer is new, send an advertisement
                events.push(Event::SendAdvertisement {
                    addressed_to: AddressedTo::WireguardAddress,
                    to: src_addr,
                    wg_ip: self.wg_ip,
                });
            } else {
                warn!(target: "advertisement", "Received advertisement with old publy key => Reject");
            }
        } else {
            info!(target: "advertisement", "Advertisement from existing peer {}", src_addr);

            //                    let mut need_wg_conf_update = false;
            //
            //                     if dp.dp_visible_wg_endpoint.is_none() {
            //                         // TODO: is a no-op currently
            //                         // Get endpoint from old entry
            //                         dp.dp_visible_wg_endpoint = entry.get_mut().dp_visible_wg_endpoint.take();
            //
            //                         // if still not known, then ask wireguard
            //                         if dp.dp_visible_wg_endpoint.is_none() {
            //                             events.push(Event::ReadWireguardConfiguration);
            //                         }
            //                     }
            //
            //                     if dp.local_reachable_wg_endpoint.is_some() {
            //                         if entry.get().local_reachable_wg_endpoint.is_none() {
            //                             need_wg_conf_update = true;
            //                         }
            //                     } else {
            //                         dp.local_reachable_wg_endpoint =
            //                             self.local_reachable_wg_endpoint.take();
            //                     }
            //
            //                     if need_wg_conf_update {
            //                         events.push(Event::UpdateWireguardConfiguration);
            //                     }
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
    is_static_peer: Option<bool>,
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
}
impl DistantNode {
    pub fn from(ri: &RouteInfo) -> Self {
        DistantNode {
            is_static_peer: None,
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
        }
    }
    pub fn process_local_contact(&mut self, local: LocalContactPacket) {
        self.local_ip_list = Some(local.local_ip_list);
        self.local_admin_port = Some(local.local_admin_port);
        self.visible_endpoint = local.my_visible_wg_endpoint;
        self.public_key = Some(local.public_key);
    }
}
impl Node for DistantNode {
    fn peer_wireguard_configuration(&self) -> Option<Vec<String>> {
        self.public_key.as_ref().map(
            |public_key| {
            let mut lines = vec![];
            lines.push(format!("PublicKey = {}", &public_key.key));
            lines.push(format!("AllowedIPs = {}/128", map_to_ipv6(&self.wg_ip)));
            if let Some(endpoint) = self.visible_endpoint.as_ref() {
                debug!(target: "configuration", "node {} uses visible (NAT) endpoint {}", self.wg_ip, endpoint);
                debug!(target: &self.wg_ip.to_string(), "use visible (NAT) endpoint {}", endpoint);
                lines.push(format!("EndPoint = {}", endpoint));
            }
            lines
        })
    }
    fn process_every_second(
        &mut self,
        _now: u64,
        static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];

        let pk_available = if self.public_key.is_some() {
            ", public key available"
        } else {
            ""
        };
        trace!(target: "nodes", "Alive node: {:?} for {} s {}", self.wg_ip, self.known_in_s, pk_available);
        self.known_in_s += 1;

        if self.is_static_peer.is_none() {
            self.is_static_peer = Some(static_config.peers.contains_key(&self.wg_ip));
        }

        if self.is_static_peer == Some(true) {
            // nothing to do for a static peer
            //
            // static peers will be polled regularly until direct connection has been successfully
            // established.
            return events;
        }

        if self.local_ip_list.is_none()
            || self.public_key.is_none()
            || self.visible_endpoint.is_none()
        {
            if self.known_in_s % 60 == 0 || self.known_in_s < 5 {
                // Send request for local contact
                let destination = SocketAddrV4::new(self.wg_ip, self.admin_port);
                events.push(Event::SendLocalContactRequest { to: destination });
            }
        } else if let Some(admin_port) = self.local_admin_port.as_ref() {
            // All ok. so constantly send advertisement to the Ipv6 address
            events.push(Event::SendAdvertisement {
                addressed_to: AddressedTo::WireguardV6Address,
                to: SocketAddr::new(IpAddr::V6(map_to_ipv6(&self.wg_ip)), *admin_port),
                wg_ip: self.wg_ip,
            });
        }

        if self.send_count < 100 {
            if let Some(ip_list) = self.local_ip_list.as_ref() {
                if let Some(admin_port) = self.local_admin_port.as_ref() {
                    self.send_count += 1;
                    for ip in ip_list.iter() {
                        if let IpAddr::V4(ipv4) = ip {
                            if *ipv4 == self.wg_ip {
                                continue;
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
        }

        let can_send = self.public_key.is_some() && self.visible_endpoint.is_some();

        if can_send && !self.can_send_to_visible_endpoint {
            self.can_send_to_visible_endpoint = true;
            events.push(Event::UpdateWireguardConfiguration);
        }

        events
    }
    fn ok_to_delete_without_route(&self, _now: u64) -> bool {
        self.known_in_s > 10
    }
    fn analyze_advertisement(
        &mut self,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> (Option<Box<dyn Node>>, Vec<Event>) {
        let mut events = vec![];

        let dp = DynamicPeer::from_advertisement(static_config, advertisement, src_addr);

        // As this peer is new, send an advertisement
        info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
        events.push(Event::SendAdvertisement {
            addressed_to: AddressedTo::WireguardAddress,
            to: src_addr,
            wg_ip: dp.wg_ip,
        });
        events.push(Event::UpdateWireguardConfiguration);

        // if still not known, then ask wireguard
        if dp.dp_visible_wg_endpoint.is_none() {
            events.push(Event::ReadWireguardConfiguration);
        }

        (Some(Box::new(dp)), events)
    }
    fn update_from_wireguard_configuration(
        &mut self,
        pubkey_to_endpoint: &mut HashMap<String, SocketAddr>,
    ) {
        if let Some(public_key) = self.public_key.as_ref() {
            if let Some(endpoint) = pubkey_to_endpoint.remove(&public_key.key) {
                self.visible_endpoint = Some(endpoint);
            }
        }
    }
    fn local_admin_port(&self) -> u16 {
        self.admin_port
    }
}
