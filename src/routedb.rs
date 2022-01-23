use std::collections::HashMap;
use std::net::Ipv4Addr;

use log::*;
use serde::{Deserialize, Serialize};

use crate::crypt_udp::{RouteDatabasePacket};
use crate::event::Event;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RouteInfo {
    pub to: Ipv4Addr,
    pub local_admin_port: u16,
    pub hop_cnt: usize,
    pub gateway: Option<Ipv4Addr>,
}

#[derive(Default, Debug)]
pub struct PeerRouteDB {
    pub version: usize,
    nr_entries: usize,
    pub route_for: HashMap<Ipv4Addr, RouteInfo>,
}

#[derive(Default, Debug)]
pub struct RouteDBManager {
    pub routedb: Option<PeerRouteDB>,
    incoming_routedb: Option<PeerRouteDB>,
    latest_routedb_version: Option<usize>,
}
impl RouteDBManager {
    pub fn is_outdated(&self) -> bool {
        self.latest_routedb_version.is_none()
            || self.routedb.as_ref().map(|db| db.version) != self.latest_routedb_version
    }
    pub fn latest_version(&mut self, version: usize) {
        self.latest_routedb_version = Some(version);
    }
    pub fn invalidate(&mut self) {
        self.routedb = None;
        self.incoming_routedb = None;
        self.latest_routedb_version = None;
    }
    pub fn process_route_database(&mut self, req: RouteDatabasePacket) -> Vec<Event> {
        let mut need_routes_update = false;
        let mut events = vec![];
        debug!(target: "routing", "RouteDatabase: {:#?}", req.known_routes);

        // The database will be received in one to many udp packages.
        //
        if let Some(mut incoming_routedb) = self.incoming_routedb.take() {
            // route database exist and must not be complete
            //
            if req.nr_entries != incoming_routedb.nr_entries {
                if req.routedb_version == incoming_routedb.version {
                    // All ok, got more entries for the database
                    for ri in req.known_routes {
                        incoming_routedb.route_for.insert(ri.to, ri);
                    }

                    // Check, if the database is complete
                    if req.nr_entries == incoming_routedb.route_for.len() {
                        need_routes_update = true;
                        self.routedb = Some(incoming_routedb);
                    } else {
                        // put back the route_db into the incoming_routedb
                        self.incoming_routedb = Some(incoming_routedb);
                    }
                } else {
                    // Received packet with wrong version info
                    warn!(target: "routing", "Mismatch of route db version, so partial db is dropped");
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
            let incoming_routedb = PeerRouteDB {
                version: req.routedb_version,
                nr_entries: req.nr_entries,
                route_for: routes,
            };

            // Perhaps the database is already complete ?
            if req.nr_entries == incoming_routedb.route_for.len() {
                need_routes_update = true;
                self.routedb = Some(incoming_routedb);
            } else {
                // put back the route_db into the incoming_routedb
                self.incoming_routedb = Some(incoming_routedb);
            }
        }
        if need_routes_update {
            events.push(Event::UpdateRoutes);
        }
        events
    }
}
