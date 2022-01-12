//
// Any peer will send out their view of the network on request.
//
// One possibility:
//      Versioned list of (wg_ip, connnected_to_wg_ip, timestamp)
//
// On every change the version is updated and status info sent out to the direct peers
// If a peer does not know this, then it will request an updated list
//
// The NetworkManager shall provide as output
//      All info to set up routing to the network nodes with gateway information
//      wg_ip list of peers in order to request the public key and endpoints
//      proposal for trying short routes
//
// As input:
//      list of dynamic peers
//      answers from other nodes
//      status info from other nodes
//
// For testing:
//      allow multiple instances of NetworkManager, which can be connected by glue code freely
//

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use log::*;
use serde::{Deserialize, Serialize};

use crate::configuration::*;
use crate::crypt_udp::*;
use crate::event::Event;

#[derive(Debug)]
pub enum RouteChange {
    AddRoute {
        to: Ipv4Addr,
        gateway: Option<Ipv4Addr>,
    },
    ReplaceRoute {
        to: Ipv4Addr,
        gateway: Option<Ipv4Addr>,
    },
    DelRoute {
        to: Ipv4Addr,
        gateway: Option<Ipv4Addr>,
    },
}

//pub struct NodeInfo {
//    timestamp: u64,
//    public_key: Option<PublicKeyWithTime>,
//}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteInfo {
    to: Ipv4Addr,
    local_admin_port: u16,
    hop_cnt: usize,
    gateway: Option<Ipv4Addr>,
}

#[derive(Default)]
pub struct RouteDB {
    version: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
}

#[derive(Default)]
pub struct PeerRouteDB {
    version: usize,
    nr_entries: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
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
    pub dp_visible_admin_endpoint: Option<SocketAddr>,
    pub admin_port: u16,
    pub lastseen: u64,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    pub my_visible_admin_endpoint: Option<SocketAddr>,
    pub my_visible_wg_endpoint: Option<SocketAddr>,
    route_db: RouteDB,
    peer_route_db: HashMap<Ipv4Addr, PeerRouteDB>,

    pending_route_changes: Vec<RouteChange>,
    new_nodes: Vec<SocketAddrV4>,
    peer: HashMap<Ipv4Addr, DynamicPeer>,
    fifo_dead: Vec<Ipv4Addr>,
    fifo_ping: Vec<Ipv4Addr>,
}

impl NetworkManager {
    pub fn new(wg_ip: Ipv4Addr) -> Self {
        NetworkManager {
            wg_ip,
            //all_nodes: HashMap::new(),
            my_visible_admin_endpoint: None,
            my_visible_wg_endpoint: None,
            route_db: RouteDB::default(),
            peer_route_db: HashMap::new(),
            pending_route_changes: vec![],
            new_nodes: vec![],
            peer: HashMap::new(),
            fifo_dead: vec![],
            fifo_ping: vec![],
        }
    }

    pub fn remove_dynamic_peer(&mut self, peer_ip: &Ipv4Addr) {
        self.peer.remove(peer_ip);
        self.recalculate_routes();
    }

    pub fn get_route_changes(&mut self) -> Vec<RouteChange> {
        let mut routes = vec![];
        routes.append(&mut self.pending_route_changes);
        routes
    }
    pub fn db_version(&self) -> usize {
        self.route_db.version
    }
    pub fn stats(&self) {
        trace!(
            "Manager: {} peers, {} in network",
            self.peer.len(),
            self.route_db.route_for.len()
        );
    }
    pub fn peer_iter(&self) -> std::collections::hash_map::Values<Ipv4Addr, DynamicPeer> {
        self.peer.values()
    }
    pub fn analyze_advertisement(
        &mut self,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> Vec<Event> {
        let mut events = vec![];

        self.fifo_dead.retain(|ip| *ip != advertisement.wg_ip);
        self.fifo_dead.retain(|ip| *ip != advertisement.wg_ip);
        self.fifo_dead.push(advertisement.wg_ip);
        self.fifo_ping.push(advertisement.wg_ip);
        let lastseen = crate::util::now();

        let mut dp_visible_admin_endpoint = None;
        let mut local_reachable_admin_endpoint = None;
        let mut local_reachable_wg_endpoint = None;

        use AddressedTo::*;
        match &advertisement.addressed_to {
            StaticAddress | VisibleAddress => {
                dp_visible_admin_endpoint = Some(src_addr);
            }
            LocalAddress | ReplyFromLocalAddress => {
                local_reachable_wg_endpoint = Some(SocketAddr::new(src_addr.ip(), advertisement.local_wg_port));
                local_reachable_admin_endpoint = Some(src_addr);
            }
            ReplyFromStaticAddress | ReplyFromVisibleAddress => {
                if advertisement.your_visible_admin_endpoint.is_some() {
                    self.my_visible_admin_endpoint = advertisement.your_visible_admin_endpoint;
                }
            }
            WireguardAddress | ReplyFromWireguardAddress => {
                if advertisement.your_visible_admin_endpoint.is_some() {
                    self.my_visible_admin_endpoint = advertisement.your_visible_admin_endpoint;
                }
                if advertisement.your_visible_wg_endpoint.is_some() {
                    self.my_visible_wg_endpoint = advertisement.your_visible_wg_endpoint;
                }
            }
        }

        let mut dp = DynamicPeer {
            wg_ip: advertisement.wg_ip,
            local_admin_port: advertisement.local_admin_port,
            local_wg_port: advertisement.local_wg_port,
            public_key: advertisement.public_key.clone(),
            name: advertisement.name.to_string(),
            local_reachable_admin_endpoint,
            local_reachable_wg_endpoint,
            dp_visible_admin_endpoint,
            dp_visible_wg_endpoint: None,
            admin_port: src_addr.port(),
            lastseen,
        };
        match self.peer.entry(advertisement.wg_ip) {
            Entry::Occupied(mut entry) => {
                // Check if public_key including creation time is same
                if entry.get().public_key != advertisement.public_key {
                    // Different public_key. Accept the one from advertisement only, if not older
                    if entry.get().public_key.priv_key_creation_time
                        <= advertisement.public_key.priv_key_creation_time
                    {
                        info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
                        events.push(Event::UpdateWireguardConfiguration);

                        // As this peer is new, send an advertisement
                        events.push(Event::SendAdvertisement {
                            addressed_to: AddressedTo::WireguardAddress,
                            to: src_addr,
                            wg_ip: dp.wg_ip,
                        });
                    } else {
                        warn!(target: "advertisement", "Received advertisement with old publy key => Reject");
                    }
                } else {
                    info!(target: "advertisement", "Advertisement from existing peer {}", src_addr);

                    if dp.dp_visible_wg_endpoint.is_none() { // TODO: is a no-op currently
                        // Get endpoint from old entry
                        dp.dp_visible_wg_endpoint = entry.get_mut().dp_visible_wg_endpoint.take();

                        // if still not known, then ask wireguard
                        if dp.dp_visible_wg_endpoint.is_none() {
                            events.push(Event::ReadWireguardConfiguration);
                        }
                    }

                    if dp.dp_visible_admin_endpoint.is_none() {
                        // Get endpoint from old entry
                        dp.dp_visible_admin_endpoint =
                            entry.get_mut().dp_visible_admin_endpoint.take();
                    }
                    
                    if dp.local_reachable_wg_endpoint.is_none() {
                        dp.local_reachable_wg_endpoint = entry.get_mut().local_reachable_wg_endpoint.take();
                    }

                    if dp.local_reachable_admin_endpoint.is_none() {
                        dp.local_reachable_admin_endpoint = entry.get_mut().local_reachable_admin_endpoint.take();
                    }

                }
                entry.insert(dp);
            }
            Entry::Vacant(entry) => {
                use AddressedTo::*;

                info!(target: "advertisement", "Advertisement from new peer {}", src_addr);
                events.push(Event::UpdateWireguardConfiguration);

                // Answers to advertisments are only sent, if the wireguard ip is not
                // in the list of dynamic peers and as such is new.
                // Consequently the reply is sent over the internet and not via
                // wireguard tunnel, because that tunnel is not yet set up.
                let addressed_to = match advertisement.addressed_to {
                    StaticAddress => { ReplyFromStaticAddress }
                    VisibleAddress => { ReplyFromVisibleAddress }
                    LocalAddress => { ReplyFromLocalAddress }
                    WireguardAddress => { ReplyFromLocalAddress }

                    replies => replies,
                };
                events.push(Event::SendAdvertisement {
                    addressed_to,
                    to: src_addr,
                    wg_ip: dp.wg_ip,
                });

                // store the dynamic peer
                entry.insert(dp);

                self.recalculate_routes();
                events.push(Event::UpdateRoutes);

                // indirectly inform about route database update
                events.push(Event::SendPingToAllDynamicPeers);
            }
        }

        if let Some(peer_route_db) = self.peer_route_db.get(&advertisement.wg_ip) {
            if peer_route_db.version != advertisement.routedb_version {
                // need to request new route database via tunnel
                info!(target: "routing", "Request updated route database from peer {}", src_addr);
                self.peer_route_db.remove(&advertisement.wg_ip);
                let destination = SocketAddrV4::new(advertisement.wg_ip, src_addr.port());
                events.push(Event::SendRouteDatabaseRequest { to: destination });
            }
        } else {
            info!(target: "routing", "Request new route database from peer {}", src_addr);
            let destination = SocketAddrV4::new(advertisement.wg_ip, src_addr.port());
            events.push(Event::SendRouteDatabaseRequest { to: destination });
        }

        while let Some(sa) = self.new_nodes.pop() {
            events.push(Event::SendLocalContactRequest { to: sa });
        }

        events
    }
    pub fn provide_route_database(&self) -> Vec<UdpPacket> {
        let mut known_routes = vec![];
        for ri in self.route_db.route_for.values() {
            known_routes.push(ri);
        }
        let p = UdpPacket::make_route_database(
            self.wg_ip,
            self.route_db.version,
            known_routes.len(),
            known_routes,
        );
        vec![p]
    }
    pub fn process_route_database(&mut self, req: RouteDatabasePacket) -> Vec<Event> {
        let mut need_routes_update = false;
        let mut events = vec![];
        debug!(target: "routing", "RouteDatabase: {:#?}", req.known_routes);

        // After requesting route database, then the old one has been deleted.
        // The database will be received in one to many udp packages.
        //
        if let Some(mut peer_route_db) = self.peer_route_db.remove(&req.sender) {
            // route database exist and must not be complete
            //
            if req.nr_entries != peer_route_db.nr_entries {
                if req.routedb_version == peer_route_db.version {
                    // All ok, got more entries for the database
                    for ri in req.known_routes {
                        peer_route_db.route_for.insert(ri.to, ri);
                    }

                    // Check, if the database is complete
                    need_routes_update = req.nr_entries == peer_route_db.route_for.len();

                    // put back the route_db into the peer_route_db
                    self.peer_route_db.insert(req.sender, peer_route_db);
                } else {
                    // Received packet with wrong version info
                    warn!(target: "routing", "Mismatch of route db versioni, so partial db is dropped");
                }
            } else {
                warn!(target: "routing", "Another packet for complete database received, so strange db is dropped");
            }
        } else {
            // First packet of a new route_db. So just store it
            let routes = req
                .known_routes
                .iter()
                .map(|e| (e.to, e.clone()))
                .collect::<HashMap<Ipv4Addr, RouteInfo>>();
            let peer_route_db = PeerRouteDB {
                version: req.routedb_version,
                nr_entries: req.nr_entries,
                route_for: routes,
            };

            // Perhaps the database is already complete ?
            need_routes_update = req.nr_entries == peer_route_db.route_for.len();

            // Store the route_db into the peer_route_db
            self.peer_route_db.insert(req.sender, peer_route_db);
        }
        if need_routes_update {
            self.recalculate_routes();
            events.push(Event::UpdateRoutes);
        }
        events
    }
    pub fn process_local_contact(&self, local: LocalContactPacket) -> Vec<Event> {
        // Send advertisement to all local addresses
        let mut events: Vec<Event> = local
            .local_ip_list
            .iter()
            .filter(|ip| match *ip {
                IpAddr::V4(ipv4) => *ipv4 != local.wg_ip,
                IpAddr::V6(_) => true,
            })
            .map(|ip| Event::SendAdvertisement {
                addressed_to: AddressedTo::LocalAddress,
                to: SocketAddr::new(*ip, local.local_admin_port),
                wg_ip: local.wg_ip,
            })
            .collect();

        // Send advertisement to a possible visible endpoint
        if local.my_visible_wg_endpoint.is_some() {
            if let Some(admin_endpoint) = local.my_visible_admin_endpoint {
                events.push( Event::SendAdvertisement {
                    addressed_to: AddressedTo::VisibleAddress,
                    to: admin_endpoint,
                    wg_ip: local.wg_ip,
                });
            }
        }

        events
    }
    fn recalculate_routes(&mut self) {
        trace!(target: "routing", "Recalculate routes");
        // Use as input:
        //    list of peers (being alive)
        //    peer route_db, if valid
        let mut new_routes: HashMap<Ipv4Addr, RouteInfo> = HashMap::new();

        // Dynamic peers are ALWAYS reachable without a gateway
        for dp in self.peer.values() {
            trace!(target: "routing", "Include direct path to dynamic peer to new routes: {}", dp.wg_ip);
            let ri = RouteInfo {
                to: dp.wg_ip,
                local_admin_port: dp.local_admin_port,
                hop_cnt: 0,
                gateway: None,
            };
            new_routes.insert(dp.wg_ip, ri);
        }
        for (wg_ip, peer_route_db) in self.peer_route_db.iter() {
            if peer_route_db.nr_entries == peer_route_db.route_for.len() {
                // is valid database for peer
                trace!(target: "routing", "consider valid database of {}: {:#?}", wg_ip, peer_route_db.route_for);
                for ri in peer_route_db.route_for.values() {
                    // Ignore routes to myself
                    if ri.to == self.wg_ip {
                        trace!(target: "routing", "Route to myself => ignore");
                        continue;
                    }
                    // Ignore routes to my dynamic peers
                    if self.peer.contains_key(&ri.to) {
                        trace!(target: "routing", "Route to any of my peers => ignore");
                        continue;
                    }
                    let mut hop_cnt = 1;
                    if let Some(gateway) = ri.gateway.as_ref() {
                        hop_cnt = ri.hop_cnt + 1;

                        // Ignore routes to myself as gateway
                        if *gateway == self.wg_ip {
                            trace!(target: "routing", "Route to myself as gateway => ignore");
                            continue;
                        }
                        if self.peer.contains_key(gateway) {
                            trace!(target: "routing", "Route using any of my peers as gateway => ignore");
                            continue;
                        }
                        if self.peer_route_db.contains_key(gateway) {
                            error!(target: "routing", "Route using any of my peers as gateway => ignore (should not come here)");
                            continue;
                        }
                    }
                    // to-host can be reached via wg_ip
                    trace!(target: "routing", "Include to routes: {} via {:?} and hop_cnt {}", ri.to, wg_ip, hop_cnt);
                    let ri_new = RouteInfo {
                        to: ri.to,
                        local_admin_port: ri.local_admin_port,
                        hop_cnt,
                        gateway: Some(*wg_ip),
                    };
                    match new_routes.entry(ri.to) {
                        Entry::Vacant(e) => {
                            e.insert(ri_new);
                        }
                        Entry::Occupied(mut e) => {
                            let current = e.get_mut();
                            if current.hop_cnt > ri_new.hop_cnt {
                                // new route is better, so replace
                                *current = ri_new;
                            } 
                        }
                    }
                }
            } else {
                warn!(target: "routing", "incomplete database from {} => ignore", wg_ip);
            }
        }

        for entry in new_routes.iter() {
            debug!(target: "routing", "new routes' entry: {:?}", entry);
        }
        for ri in self.route_db.route_for.values_mut() {
            debug!(target: "routing", "Existing route: {:?}", ri);
        }

        // new_routes is built. So update route_db and mark changes
        //
        // first routes to be deleted
        for ri in self.route_db.route_for.values_mut() {
            if !new_routes.contains_key(&ri.to) {
                trace!(target: "routing", "del route {:?}", ri);
                self.pending_route_changes.push(RouteChange::DelRoute {
                    to: ri.to,
                    gateway: ri.gateway,
                });
            } else {
                trace!(target: "routing", "unchanged route {:?}", ri);
            }
        }
        // finally routes to be updated / added
        for (to, ri) in new_routes.into_iter() {
            trace!(target: "routing", "process route {} via {:?}", to, ri.gateway);
            match self.route_db.route_for.entry(to) {
                Entry::Vacant(e) => {
                    // new node with route
                    trace!(target: "routing", "is new route {} via {:?}", to, ri.gateway);
                    self.pending_route_changes.push(RouteChange::AddRoute {
                        to,
                        gateway: ri.gateway,
                    });
                    if ri.gateway.is_some() {
                        info!(target: "probing", "detected a new host {} via {}", to, ri.gateway.as_ref().unwrap());
                        let sa = SocketAddrV4::new(to, ri.local_admin_port);
                        self.new_nodes.push(sa);
                    }
                    let mut ri_new = RouteInfo {
                        to,
                        local_admin_port: ri.local_admin_port,
                        hop_cnt: ri.hop_cnt,
                        gateway: ri.gateway,
                    };
                    if ri.gateway.is_some() {
                        ri_new.hop_cnt += 1;
                    }
                    e.insert(ri_new);
                }
                Entry::Occupied(mut e) => {
                    // update route
                    if e.get().to != to {
                        trace!(target: "routing", "replace existing route {}", to);
                        self.pending_route_changes.push(RouteChange::ReplaceRoute {
                            to,
                            gateway: ri.gateway,
                        });
                        *e.get_mut() = RouteInfo {
                            to,
                            local_admin_port: ri.local_admin_port,
                            hop_cnt: ri.hop_cnt,
                            gateway: ri.gateway,
                        };
                    }
                }
            }
            trace!(target: "routing", "route changes: {}", self.pending_route_changes.len());
        }
        if !self.pending_route_changes.is_empty() {
            trace!(target: "routing", "{} route changes", self.pending_route_changes.len());
            self.route_db.version += 1;
        }
    }
    pub fn get_ips_for_peer(&self, peer: Ipv4Addr) -> Vec<Ipv4Addr> {
        let mut ips = vec![];

        for ri in self.route_db.route_for.values() {
            if ri.gateway == Some(peer) {
                ips.push(ri.to);
            }
        }

        ips
    }
    pub fn dynamic_peer_for(&mut self, wg_ip: &Ipv4Addr) -> Option<&DynamicPeer> {
        self.peer.get(wg_ip)
    }
    pub fn knows_peer(&mut self, wg_ip: &Ipv4Addr) -> bool {
        self.peer.contains_key(wg_ip)
    }
    pub fn output(&self) {
        for peer in self.peer.values() {
            debug!(target: "active_peers", "{:?}", peer);
        }
    }
    pub fn check_timeouts(&mut self, limit: u64) -> HashSet<Ipv4Addr> {
        let mut dead_peers = HashSet::new();
        let now = crate::util::now();
        while let Some(wg_ip) = self.fifo_dead.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                let dt = now - peer.lastseen;
                trace!(target: "dead_peer", "Peer {} last seen {} s before, limit = {}. fifo_dead = {:?}", wg_ip, dt, limit, self.fifo_dead);
                if dt < limit {
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
        let now = crate::util::now();
        while let Some(wg_ip) = self.fifo_ping.first().as_ref() {
            if let Some(peer) = self.peer.get(*wg_ip) {
                let dt = now - peer.lastseen;
                if dt < limit {
                    break;
                }
                ping_peers.insert((**wg_ip, peer.admin_port));
            }
            self.fifo_ping.remove(0);
        }
        ping_peers
    }
    pub fn current_wireguard_configuration(
        &mut self,
        mut pubkey_to_endpoint: HashMap<String, SocketAddr>,
    ) {
        for dynamic_peer in self.peer.values_mut() {
            if let Some(endpoint) = pubkey_to_endpoint.remove(&dynamic_peer.public_key.key) {
                dynamic_peer.dp_visible_wg_endpoint = Some(endpoint);
                // The dp_visible_admin port may be different, so do not derive it
            }
        }
    }
}
