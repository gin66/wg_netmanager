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
use std::net::{Ipv4Addr, SocketAddr};

use log::*;
use serde::{Deserialize, Serialize};

use crate::configuration::*;
use crate::crypt_udp::*;
use crate::event::Event;
use crate::node::*;

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteInfo {
    pub to: Ipv4Addr,
    pub local_admin_port: u16,
    hop_cnt: usize,
    gateway: Option<Ipv4Addr>,
}

#[derive(Default, Debug)]
pub struct RouteDB {
    version: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
}

#[derive(Default, Debug)]
pub struct PeerRouteDB {
    pub version: usize,
    nr_entries: usize,
    route_for: HashMap<Ipv4Addr, RouteInfo>,
}

pub struct NetworkManager {
    wg_ip: Ipv4Addr,
    pub my_visible_wg_endpoint: Option<SocketAddr>,
    route_db: RouteDB,
    peer_route_db: HashMap<Ipv4Addr, PeerRouteDB>,
    pending_route_changes: Vec<RouteChange>,
    pub all_nodes: HashMap<Ipv4Addr, Box<dyn Node>>,
}

impl NetworkManager {
    pub fn new(static_config: &StaticConfiguration) -> Self {
        let all_nodes = static_config
            .peers
            .iter()
            .filter(|(wg_ip, _)| **wg_ip != static_config.wg_ip)
            .map(|(wg_ip, peer)| (*wg_ip, StaticPeer::from_public_peer(peer)))
            .collect::<HashMap<Ipv4Addr, Box<dyn Node>>>();

        NetworkManager {
            wg_ip: static_config.wg_ip,
            my_visible_wg_endpoint: None,
            route_db: RouteDB::default(),
            peer_route_db: HashMap::new(),
            pending_route_changes: vec![],
            all_nodes,
        }
    }

    pub fn get_route_changes(&mut self) -> Vec<RouteChange> {
        self.recalculate_routes();
        let mut routes = vec![];
        routes.append(&mut self.pending_route_changes);
        routes
    }
    pub fn db_version(&self) -> usize {
        self.route_db.version
    }
    pub fn stats(&self) {
        trace!("Manager: {} nodes in network", self.all_nodes.len(),);
    }
    pub fn analyze_advertisement(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
        advertisement: AdvertisementPacket,
        src_addr: SocketAddr,
    ) -> Vec<Event> {
        if let Some(endpoint) = advertisement.your_visible_wg_endpoint.as_ref() {
            // Could be more than one
            self.my_visible_wg_endpoint = Some(*endpoint);
        }

        match self.all_nodes.entry(advertisement.wg_ip) {
            Entry::Occupied(mut entry) => {
                let now = crate::util::now();
                let (opt_new_entry, events) = entry.get_mut().analyze_advertisement(
                    now,
                    static_config,
                    advertisement,
                    src_addr,
                );
                if let Some(new_entry) = opt_new_entry {
                    entry.insert(new_entry);
                }
                events
            }
            Entry::Vacant(entry) => {
                let mut events = vec![];
                info!(target: "advertisement", "Advertisement from new peer {}", src_addr);

                events.push(Event::UpdateWireguardConfiguration);

                // Answers to advertisments are only sent, if the wireguard ip is not
                // in the list of dynamic peers and as such is new.
                // Consequently the reply is sent over the internet and not via
                // wireguard tunnel, because that tunnel is not yet set up.
                events.push(Event::SendAdvertisement {
                    addressed_to: advertisement.addressed_to.reply(),
                    to: src_addr,
                    wg_ip: self.wg_ip,
                });
                events.push(Event::UpdateRoutes);

                let dp =
                    DynamicPeer::from_advertisement(now, static_config, advertisement, src_addr);
                entry.insert(Box::new(dp));

                events
            }
        }
    }
    pub fn process_all_nodes_every_second(
        &mut self,
        now: u64,
        static_config: &StaticConfiguration,
    ) -> Vec<Event> {
        let mut events = vec![];
        let mut node_to_delete = vec![];
        for (node_wg_ip, node) in self.all_nodes.iter_mut() {
            //    if !self.route_db.route_for.contains_key(node_wg_ip) {
            // have no route to this peer
            if node.ok_to_delete_without_route(now) {
                node_to_delete.push(*node_wg_ip);
                continue;
            }
            //    }
            let mut new_events = node.process_every_second(now, static_config);
            events.append(&mut new_events);
        }

        if !node_to_delete.is_empty() {
            events.push(Event::UpdateWireguardConfiguration);
            events.push(Event::UpdateRoutes);

            for wg_ip in node_to_delete {
                debug!(target: &wg_ip.to_string(), "is dead => remove");
                debug!(target: "dead_peer", "Found dead peer {}", wg_ip);
                self.all_nodes.remove(&wg_ip);
            }
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
    pub fn process_local_contact(&mut self, local: LocalContactPacket) {
        // Send advertisement to all local addresses
        debug!(target: &local.wg_ip.to_string(), "LocalContact: {:#?}", local);
        let wg_ip = local.wg_ip;
        if let Some(node) = self.all_nodes.get_mut(&wg_ip) {
            node.process_local_contact(local);
        }
    }
    fn recalculate_routes(&mut self) {
        trace!(target: "routing", "Recalculate routes");
        // Use as input:
        //    list of peers (being alive)
        //    peer route_db, if valid
        let mut new_routes: HashMap<Ipv4Addr, RouteInfo> = HashMap::new();

        // Dynamic peers are ALWAYS reachable without a gateway
        for (wg_ip, node) in self.all_nodes.iter() {
            trace!(target: "routing", "Include direct path to dynamic peer to new routes: {}", wg_ip);
            let ri = RouteInfo {
                to: *wg_ip,
                local_admin_port: node.local_admin_port(),
                hop_cnt: 0,
                gateway: None,
            };
            new_routes.insert(*wg_ip, ri);
        }
        //        for (wg_ip, peer_route_db) in self.peer_route_db.iter() {
        //            if peer_route_db.nr_entries == peer_route_db.route_for.len() {
        //                // is valid database for peer
        //                trace!(target: "routing", "consider valid database of {}: {:#?}", wg_ip, peer_route_db.route_for);
        //                for ri in peer_route_db.route_for.values() {
        //                    // Ignore routes to myself
        //                    if ri.to == self.wg_ip {
        //                        trace!(target: "routing", "Route to myself => ignore");
        //                        continue;
        //                    }
        //                    // Ignore routes to my dynamic peers
        //                    if self.peer.contains_key(&ri.to) {
        //                        trace!(target: "routing", "Route to any of my peers => ignore");
        //                        continue;
        //                    }
        //                    let mut hop_cnt = 1;
        //                    if let Some(gateway) = ri.gateway.as_ref() {
        //                        hop_cnt = ri.hop_cnt + 1;
        //
        //                        // Ignore routes to myself as gateway
        //                        if *gateway == self.wg_ip {
        //                            trace!(target: "routing", "Route to myself as gateway => ignore");
        //                            continue;
        //                        }
        //                        if self.peer.contains_key(gateway) {
        //                            trace!(target: "routing", "Route using any of my peers as gateway => ignore");
        //                            continue;
        //                        }
        //                        if self.peer_route_db.contains_key(gateway) {
        //                            error!(target: "routing", "Route using any of my peers as gateway => ignore (should not come here)");
        //                            continue;
        //                        }
        //                    }
        //                    // to-host can be reached via wg_ip
        //                    trace!(target: "routing", "Include to routes: {} via {:?} and hop_cnt {}", ri.to, wg_ip, hop_cnt);
        //                    let ri_new = RouteInfo {
        //                        to: ri.to,
        //                        local_admin_port: ri.local_admin_port,
        //                        hop_cnt,
        //                        gateway: Some(*wg_ip),
        //                    };
        //                    match new_routes.entry(ri.to) {
        //                        Entry::Vacant(e) => {
        //                            e.insert(ri_new);
        //                        }
        //                        Entry::Occupied(mut e) => {
        //                            let current = e.get_mut();
        //                            if current.hop_cnt > ri_new.hop_cnt {
        //                                // new route is better, so replace
        //                                *current = ri_new;
        //                            }
        //                        }
        //                    }
        //                    if let Entry::Vacant(e) = self.known_nodes.entry(ri.to) {
        //                        info!(target: "probing", "detected a new node {} via {:?}", ri.to, ri.gateway);
        //                        let node = DistantNode::from(ri);
        //                        e.insert(node);
        //                    }
        //                }
        //            } else {
        //                warn!(target: "routing", "incomplete database from {} => ignore", wg_ip);
        //            }
        //        }

        for entry in new_routes.iter() {
            debug!(target: "routing", "new routes' entry: {:?}", entry);
        }
        for ri in self.route_db.route_for.values_mut() {
            debug!(target: "routing", "Existing route: {:?}", ri);
        }

        // new_routes is built. So update route_db and mark changes
        //
        // first routes to be deleted
        let mut to_be_deleted = vec![];
        for ri in self.route_db.route_for.values_mut() {
            if !new_routes.contains_key(&ri.to) {
                trace!(target: "routing", "del route {:?}", ri);
                self.pending_route_changes.push(RouteChange::DelRoute {
                    to: ri.to,
                    gateway: ri.gateway,
                });

                to_be_deleted.push(ri.to);

                // and delete from the known_nodes.
                //self.known_nodes.remove(&ri.to);
            } else {
                trace!(target: "routing", "unchanged route {:?}", ri);
            }
        }
        for wg_ip in to_be_deleted.into_iter() {
            self.route_db.route_for.remove(&wg_ip);
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
                    if e.get().to != ri.to || e.get().gateway != ri.gateway {
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
            for change in self.pending_route_changes.iter() {
                trace!(target: "routing", "route changes {:?}", change);
            }
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
    pub fn node_for(&mut self, wg_ip: &Ipv4Addr) -> Option<&dyn Node> {
        self.all_nodes.get(wg_ip).map(|n| n.as_ref())
    }
    pub fn knows_peer(&mut self, wg_ip: &Ipv4Addr) -> bool {
        self.all_nodes.contains_key(wg_ip)
    }
    pub fn output(&self) {
        for wg_ip in self.all_nodes.keys() {
            debug!(target: "nodes", "{:?}", wg_ip);
        }
    }
    pub fn current_wireguard_configuration(
        &mut self,
        mut pubkey_to_endpoint: HashMap<String, SocketAddr>,
    ) {
        for node in self.all_nodes.values_mut() {
            node.update_from_wireguard_configuration(&mut pubkey_to_endpoint);
        }
    }
}
