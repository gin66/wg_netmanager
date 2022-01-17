#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::Ipv4Addr;

    use log::*;

    use wg_netmanager::configuration::*;
    use wg_netmanager::crypt_udp::*;
    use wg_netmanager::manager::*;
    use wg_netmanager::event::*;

    fn get_test_config() -> StaticConfiguration {
        StaticConfiguration {
            name: "Test".to_string(),
            ip_list: vec![],
            wg_name: "wg_test".to_string(),
            wg_ip: "10.1.1.1".parse().unwrap(),
            wg_port: 50000,
            admin_port: 50001,
            subnet: "10.1.1.1/8".parse().unwrap(),
            shared_key: vec![],
            my_private_key: "".to_string(),
            my_public_key: PublicKeyWithTime {
                key: "".to_string(),
                priv_key_creation_time: 0,
            },
            peers: HashMap::new(),
            peer_cnt: 0,
            use_tui: false,
            use_existing_interface: false,
            network_yaml_filename: "".to_string(),
            peer_yaml_filename: None,
        }
    }

    #[test]
    fn test_make_manager() {
        let config = get_test_config();
        let mut mgr = NetworkManager::new(&config);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }

    #[test]
    fn test_with_one_dynamic_peer() {
        wg_netmanager::error::set_up_logging(log::LevelFilter::Trace, None);

        let peer_ip: Ipv4Addr = "10.1.1.2".parse().unwrap();

        let public_key = PublicKeyWithTime {
            key: "".to_string(),
            priv_key_creation_time: 0,
        };
        let static_config = StaticConfiguration {
            name: "test".to_string(),
            ip_list: vec![],
            wg_ip: "10.1.1.1".parse().unwrap(),
            wg_name: "wg0".to_string(),
            wg_port: 55555,
            admin_port: 50000,
            subnet: "192.168.1.1/24".parse().unwrap(),
            shared_key: vec![],
            my_private_key: "".to_string(),
            my_public_key: public_key.clone(),
            peers: HashMap::new(),
            peer_cnt: 1,
            use_tui: false,
            use_existing_interface: true,
            network_yaml_filename: "".to_string(),
            peer_yaml_filename: None,
        };
        let mut mgr = NetworkManager::new(&static_config);

        let ad = AdvertisementPacket {
            addressed_to: AddressedTo::StaticAddress,
            public_key,
            local_wg_port: 0,
            local_admin_port: 0,
            wg_ip: peer_ip,
            name: "test".to_string(),
            your_visible_wg_endpoint: Some("192.168.1.1:1".parse().unwrap()),
            my_visible_wg_endpoint: Some("192.168.1.2:1".parse().unwrap()),
            routedb_version: 0,
        };
        let events =
            mgr.analyze_advertisement(&static_config, ad, "192.168.1.1:2".parse().unwrap());

        trace!("{:#?}", events);
        for evt in events {
            match evt {
                Event::UpdateRoutes => {
            }
                _ => {}
            }
        }

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
        for _ in 1..200 {
            mgr.process_all_nodes_every_second(&static_config);
        }
        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }
}
