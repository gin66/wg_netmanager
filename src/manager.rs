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
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

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
pub struct Gateway {
    hop_cnt: usize,
    ip: Ipv4Addr,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteInfo {
    to: Ipv4Addr,
    hop_cnt: usize,
    gateway: Option<Gateway>,
}
impl RouteInfo {
    pub fn gw_ip(&self) -> Option<Ipv4Addr> {
        self.gateway.as_ref().map(|gw| gw.ip)
    }
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
    pub endpoint: Option<SocketAddr>,
    pub admin_port: u16,
    pub lastseen: u64,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    route_db: RouteDB,
    peer_route_db: HashMap<Ipv4Addr, PeerRouteDB>,

    pending_route_changes: Vec<RouteChange>,

    peer: HashMap<Ipv4Addr, DynamicPeer>,
    fifo_dead: Vec<Ipv4Addr>,
    fifo_ping: Vec<Ipv4Addr>,
}

impl NetworkManager {
    pub fn new(wg_ip: Ipv4Addr) -> Self {
        NetworkManager {
            wg_ip,
            //all_nodes: HashMap::new(),
            route_db: RouteDB::default(),
            peer_route_db: HashMap::new(),
            pending_route_changes: vec![],
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
        let dp = DynamicPeer {
            wg_ip: advertisement.wg_ip,
            local_admin_port: advertisement.local_admin_port,
            local_wg_port: advertisement.local_wg_port,
            public_key: advertisement.public_key.clone(),
            name: advertisement.name.to_string(),
            endpoint: advertisement.endpoint,
            admin_port: src_addr.port(),
            lastseen,
        };
        if let Some(old_peer_info) = self.peer.insert(advertisement.wg_ip, dp) {
            // Eventually the peer has been restarted, then the priv_key_creation_time must be
            // greater and the public key in wireguard device needs to be replaced.
            if old_peer_info.public_key.priv_key_creation_time
                < advertisement.public_key.priv_key_creation_time
            {
                info!(target: "advertisement", "Advertisement from new peer at old address: {}", src_addr);
                events.push(Event::PeerListChange);

                // As this peer is new, send an advertisement
                events.push(Event::SendAdvertisement { to: src_addr });
            } else {
                info!(target: "advertisement", "Advertisement from existing peer {}", src_addr);
            }
        } else {
            info!(target: "advertisement", "Advertisement from new peer {}", src_addr);
            events.push(Event::PeerListChange);

            // Answers to advertisments are only sent, if the wireguard ip is not
            // in the list of dynamic peers and as such is new.
            // Consequently the reply is sent over the internet and not via
            // wireguard tunnel, because that tunnel is not yet set up.
            events.push(Event::SendAdvertisement { to: src_addr });

            self.recalculate_routes();
            events.push(Event::UpdateRoutes);
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
    fn recalculate_routes(&mut self) {
        trace!(target: "routing", "Recalculate routes");
        // Use as input:
        //    list of peers (being alive)
        //    peer route_db, if valid
        let mut new_routes: HashMap<Ipv4Addr, RouteInfo> = HashMap::new();

        // Dynamic peers are ALWAYS reachable without a gateway
        for (peer, _) in self.peer.iter() {
            trace!(target: "routing", "Include to routes: {}", peer);
            let ri = RouteInfo { to: *peer, hop_cnt: 0, gateway: None };
            new_routes.insert(*peer, ri);
        }
        for (wg_ip, peer_route_db) in self.peer_route_db.iter() {
            if peer_route_db.nr_entries == peer_route_db.route_for.len() {
                // is valid database for peer
                debug!(target: "routing", "valid database of {}: {:#?}", wg_ip, peer_route_db.route_for);
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
                        hop_cnt = gateway.hop_cnt + 1;

                        // Ignore routes to myself as gateway
                        if gateway.ip == self.wg_ip {
                            trace!(target: "routing", "Route to myself as gateway => ignore");
                            continue;
                        }
                        if self.peer.contains_key(&gateway.ip) {
                            trace!(target: "routing", "Route using any of my peers as gateway => ignore");
                            continue;
                        }
                        if self.peer_route_db.contains_key(&gateway.ip) {
                            error!(target: "routing", "Route using any of my peers as gateway => ignore (should not come here)");
                            continue;
                        }
                    }
                    // to-host can be reached via wg_ip
                    trace!(target: "routing", "Include to routes: {} via {:?} and hop_cnt {}", ri.to, wg_ip, hop_cnt);
                    let gateway = Gateway {
                        ip: *wg_ip,
                        hop_cnt,
                    };
                    let ri_new = RouteInfo { to: ri.to, hop_cnt, gateway: Some(gateway) };
                    new_routes.insert(ri.to, ri_new);
                }
            } else {
                warn!(target: "routing", "incomplete database from {} => ignore", wg_ip);
            }
        }

        for entry in new_routes.iter() {
            debug!(target: "routing", "Peer routes' entry: {:?}", entry);
        }

        // new_routes is built. So update route_db and mark changes
        //
        // first routes to be deleted
        for ri in self.route_db.route_for.values_mut() {
            if !new_routes.contains_key(&ri.to) {
                trace!(target: "routing", "add route {:?}", ri);
                self.pending_route_changes.push(RouteChange::DelRoute {
                    to: ri.to,
                    gateway: ri.gw_ip(),
                });
            } else {
                trace!(target: "routing", "unchanged route {:?}", ri);
            }
        }
        // finally routes to be updated / added
        for (to, ri) in new_routes.into_iter() {
            let ng = ri.gateway.clone().map(|mut gw| {
                gw.hop_cnt += 1;
                gw
            });
            trace!(target: "routing", "process route {} via {:?}", to, ri.gateway);
            match self.route_db.route_for.entry(to) {
                Entry::Vacant(e) => {
                    // new route
                    trace!(target: "routing", "is new route {} via {:?}", to, ri.gateway);
                    self.pending_route_changes.push(RouteChange::AddRoute {
                        to,
                        gateway: ri.gateway.as_ref().map(|gw| gw.ip),
                    });
                    let ri = RouteInfo { to, hop_cnt: ri.gateway.as_ref().map(|gw| gw.hop_cnt).unwrap_or(0),  gateway: ri.gateway };
                    e.insert(ri);
                }
                Entry::Occupied(mut e) => {
                    // update route
                    trace!(target: "routing", "is existing route {}", to);
                    let current = e.get_mut();
                    let current_hop_cnt = current.gateway.as_ref().map(|e| e.hop_cnt).unwrap_or(0);
                    let new_hop_cnt = ng.as_ref().map(|e| e.hop_cnt).unwrap_or(0);
                    if current_hop_cnt > new_hop_cnt {
                        // new route is better
                        //
                        // so first delete the old route
                        //
                        self.pending_route_changes.push(RouteChange::DelRoute {
                            to: current.to,
                            gateway: current.gw_ip(),
                        });

                        // then add the new route
                        self.pending_route_changes.push(RouteChange::AddRoute {
                            to,
                            gateway: ng.as_ref().map(|gw| gw.ip),
                        });
                        *current = RouteInfo { to, hop_cnt: new_hop_cnt, gateway: ng };
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
            if let Some(gateway) = ri.gateway.as_ref() {
                if gateway.ip == peer {
                    ips.push(ri.to);
                }
            }
        }

        ips
    }
    pub fn knows_peer(&mut self, wg_ip: &Ipv4Addr) -> bool {
        self.peer.contains_key(wg_ip)
    }
    pub fn output(&self) {
        for peer in self.peer.values() {
            info!("{:?}", peer);
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
}
