#[cfg(test)]
mod tests {
    use wg_netmanager::configuration::*;
    use wg_netmanager::unconnected::*;

    #[test]
    fn test_check_device_fail() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgx", "10.1.1.1");
        let wg_dev = WireguardDeviceLinux::init(&sc);
        let dc = wg_dev.check_device().unwrap();
        assert!(!dc);
    }

    #[test]
    fn test_bring_up_device() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgy", "10.1.1.1");
        let wg_dev = WireguardDeviceLinux::init(&sc);

        let dev_present_before = wg_dev.check_device().unwrap();
        assert!(!dev_present_before);

        wg_dev.bring_up_device().unwrap();

        let dev_present_after = wg_dev.check_device().unwrap();
        assert!(dev_present_after);

        wg_dev.take_down_device().unwrap();

        let dev_present_final = wg_dev.check_device().unwrap();
        assert!(!dev_present_final);
    }
}
