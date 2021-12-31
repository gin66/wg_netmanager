#[cfg(test)]
mod tests {
    use wg_netmanager::configuration::*;
    use wg_netmanager::wg_dev::*;

    #[test]
    fn test_check_device_fail() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgx", "10.1.1.1", "participant_ip","participant_listener_ip","pubkey","privkey");
        let wg_dev = WireguardDeviceLinux::init(&sc);
        let dc = wg_dev.check_device().unwrap();
        assert!(!dc);
    }

    #[test]
    fn test_bring_up_device() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgy", "10.1.1.1", "participant_ip","participant_listener_ip","pubkey","privkey");
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

    #[test]
    fn test_bring_up_device_with_ip() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgy", "10.1.1.1", "participant_ip","participant_listener_ip","pubkey","privkey");
        let wg_dev = WireguardDeviceLinux::init(&sc);

        let dev_present_before = wg_dev.check_device().unwrap();
        assert!(!dev_present_before);

        wg_dev.bring_up_device().unwrap();

        let dev_present_after = wg_dev.check_device().unwrap();
        assert!(dev_present_after);

        wg_dev.set_ip("10.1.1.1/32").unwrap();

        wg_dev.take_down_device().unwrap();

        let dev_present_final = wg_dev.check_device().unwrap();
        assert!(!dev_present_final);
    }

    #[test]
    fn test_bring_up_device_with_ip_and_key() {
        let sc = StaticConfiguration::new(
            Verbosity::All,
            "wgz",
            "10.1.1.1",
            "172.16.1.2",
            "172.16.1.1",
            "5jdkklXgy65sx67HJziWmHWXv49s2xxx/mUsQ9leDzk=",
            "YJ7Bbyc1KyUmMUqxODAxFDG8m84uZX495iRDzbawKkw=",
        );
        let wg_dev = WireguardDeviceLinux::init(&sc);

        let dev_present_before = wg_dev.check_device().unwrap();
        assert!(!dev_present_before);

        wg_dev.bring_up_device().unwrap();

        let dev_present_after = wg_dev.check_device().unwrap();
        assert!(dev_present_after);

        wg_dev.set_ip("10.1.1.1/32").unwrap();

        wg_dev
            .set_conf(
                r#"
        [Interface]
        PrivateKey = YJ7Bbyc1KyUmMUqxODAxFDG8m84uZX495iRDzbawKkw=
        "#,
            )
            .unwrap();

        wg_dev.take_down_device().unwrap();

        let dev_present_final = wg_dev.check_device().unwrap();
        assert!(!dev_present_final);
    }
}
