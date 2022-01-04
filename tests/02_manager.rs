#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::str::FromStr;

    use wg_netmanager::configuration::*;
    use wg_netmanager::manager::*;
    use wg_netmanager::wg_dev::*;

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
        let public_key_with_time = PublicKeyWithTime::default();
        mgr.add_dynamic_peer(&peer_ip);
        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);

        // now remove the peer
        mgr.remove_dynamic_peer(&peer_ip);
        assert_eq!(mgr.get_route_changes().len(), 1);
        assert_eq!(mgr.get_route_changes().len(), 0);
    }
}
