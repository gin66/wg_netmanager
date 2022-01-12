#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use log::*;

    //use wg_netmanager::configuration::*;
    use wg_netmanager::configuration::*;
    use wg_netmanager::crypt_udp::*;
    use wg_netmanager::manager::*;

    #[test]
    fn test_make_manager() {
        let ip: Ipv4Addr = "10.1.1.1".parse().unwrap();
        let mut mgr = NetworkManager::new(ip);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }

    #[test]
    fn test_with_one_dynamic_peer() {
        //wg_netmanager::error::set_up_logging(log::LevelFilter::Trace);

        let ip: Ipv4Addr = "10.1.1.1".parse().unwrap();
        let peer_ip: Ipv4Addr = "10.1.1.2".parse().unwrap();
        let mut mgr = NetworkManager::new(ip);

        let ad = AdvertisementPacket {
            addressed_to: AddressedTo::StaticAddress,
            public_key: PublicKeyWithTime {
                key: "".to_string(),
                priv_key_creation_time: 0,
            },
            local_wg_port: 0,
            local_admin_port: 0,
            wg_ip: peer_ip,
            name: "test".to_string(),
            your_visible_admin_endpoint: Some("192.168.1.1:1".parse().unwrap()),
            your_visible_wg_endpoint: Some("192.168.1.1:1".parse().unwrap()),
            routedb_version: 0,
        };
        let events = mgr.analyze_advertisement(ad, "192.168.1.1:2".parse().unwrap());

        trace!("{:#?}", events);

        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);

        println!("ROUTE");
        for udp in mgr.provide_route_database() {
            use UdpPacket::*;
            match udp {
                Advertisement(_) => {}
                RouteDatabaseRequest => {}
                RouteDatabase(req) => {
                    println!("{} {:?}", req.sender, req.known_routes);
                }
                LocalContactRequest => {}
                LocalContact(_) => {}
            }
        }

        // now remove the peer
        mgr.remove_dynamic_peer(&peer_ip);
        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }
}
