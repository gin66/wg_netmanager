#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    //use wg_netmanager::configuration::*;
    use wg_netmanager::manager::*;
    use wg_netmanager::configuration::*;

    #[test]
    fn test_make_manager() {
        let ip: Ipv4Addr = "10.1.1.1".parse().unwrap();
        let mut mgr = NetworkManager::new(ip);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }

    #[test]
    fn test_with_one_dynamic_peer() {
        let ip: Ipv4Addr = "10.1.1.1".parse().unwrap();
        let peer_ip: Ipv4Addr = "10.1.1.2".parse().unwrap();
        let mut mgr = NetworkManager::new(ip);
        mgr.add_dynamic_peer(&peer_ip);
        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);

        println!("ROUTE");
        for udp in mgr.provide_route_database() {
            use UdpPacket::*;
            match udp {
                Advertisement {..} => {}
                RouteDatabaseRequest {..} => { }
                RouteDatabase { sender, known_routes, routedb_version, nr_entries} => {
                    println!("{} {:?}", sender, known_routes);
                }
            }
        }

        // now remove the peer
        mgr.remove_dynamic_peer(&peer_ip);
        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }
}
