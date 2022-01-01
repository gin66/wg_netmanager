#[cfg(test)]
mod tests {
    use wg_netmanager::configuration::*;
    use wg_netmanager::wg_dev::*;
    use wg_netmanager::wg_dev_linuxkernel::*;

    fn demo_config() -> StaticConfigurationBuilder {
        StaticConfiguration::new()
            .verbosity(Verbosity::All)
            .name("test")
            .wg_ip("10.1.1.1")
            .wg_name("wgx")
            .new_participant_ip("172.16.1.2")
            .new_participant_listener_ip("172.16.1.1")
            .public_key_listener("5jdkklXgy65sx67HJziWmHWXv49s2xxx/mUsQ9leDzk=")
            .private_key_listener("YJ7Bbyc1KyUmMUqxODAxFDG8m84uZX495iRDzbawKkw=")
            .public_key_new_participant("5jdkklXgy65sx67HJziWmHWXv49s2xxx/mUsQ9leDzk=")
            .private_key_new_participant("YJ7Bbyc1KyUmMUqxODAxFDG8m84uZX495iRDzbawKkw=")
    }

    #[test]
    fn test_check_device_fail() {
        let wg_dev = WireguardDeviceLinux::init("wgtest0", Verbosity::All);
        let dc = wg_dev.check_device().unwrap();
        assert!(!dc);
    }

    #[test]
    fn test_bring_up_device() {
        let wg_dev = WireguardDeviceLinux::init("wgtest1", Verbosity::All);

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
        let wg_dev = WireguardDeviceLinux::init("wgtest2", Verbosity::All);

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
        let wg_dev = WireguardDeviceLinux::init("wgtest3", Verbosity::All);

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
