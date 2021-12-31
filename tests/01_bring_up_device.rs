#[cfg(test)]
mod tests {
    use wg_netmanager::configuration::*;
    use wg_netmanager::wg_dev::*;

    fn demo_config()->StaticConfigurationBuilder {
        StaticConfiguration::new()
            .verbosity(Verbosity::All)
            .wg_name("wgx")
            .unconnected_ip("10.1.1.1")
            .new_participant_ip("172.16.1.2")
            .new_participant_listener_ip("172.16.1.1")
            .public_key("5jdkklXgy65sx67HJziWmHWXv49s2xxx/mUsQ9leDzk=")
            .private_key("YJ7Bbyc1KyUmMUqxODAxFDG8m84uZX495iRDzbawKkw=")
    }

    #[test]
    fn test_check_device_fail() {
        let sc = demo_config().wg_name("wgtest0").build();
        let wg_dev = WireguardDeviceLinux::init(&sc);
        let dc = wg_dev.check_device().unwrap();
        assert!(!dc);
    }

    #[test]
    fn test_bring_up_device() {
        let sc = demo_config().wg_name("wgtest1").build();
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
        let sc = demo_config().wg_name("wgtest2").build();
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
        let sc = demo_config().wg_name("wgtest3").build();
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
