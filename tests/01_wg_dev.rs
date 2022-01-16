#[cfg(test)]
mod tests {
    use wg_netmanager::arch_def::*;
    use wg_netmanager::configuration::*;
    use wg_netmanager::Arch;

    #[allow(dead_code)]
    fn demo_config() -> StaticConfigurationBuilder {
        StaticConfiguration::builder()
            .name("test")
            .wg_ip("10.1.1.1".parse::<std::net::Ipv4Addr>().unwrap())
            .wg_name("wgx")
    }

    #[test]
    fn test_check_device_fail() {
        let wg_dev = Arch::get_wg_dev("wgtest0");
        let dc = wg_dev.check_device().unwrap();
        assert!(!dc);
    }

    #[test]
    fn test_create_device() {
        let wg_dev = Arch::get_wg_dev("wgtest1");

        let dev_present_before = wg_dev.check_device().unwrap();
        assert!(!dev_present_before);

        wg_dev.create_device().unwrap();

        let dev_present_after = wg_dev.check_device().unwrap();
        assert!(dev_present_after);

        wg_dev.take_down_device().unwrap();

        let dev_present_final = wg_dev.check_device().unwrap();
        assert!(!dev_present_final);
    }

    #[test]
    fn test_create_device_with_ip() {
        // let _ = wg_netmanager::error::set_up_logging(log::LevelFilter::Trace, None);

        let mut wg_dev = Arch::get_wg_dev("wgtest2");

        let _ = wg_dev.take_down_device();

        let dev_present_before = wg_dev.check_device().unwrap();
        assert!(!dev_present_before);

        wg_dev.create_device().unwrap();

        let dev_present_after = wg_dev.check_device().unwrap();
        assert!(dev_present_after);

        let subnet: ipnet::Ipv4Net = "10.202.0.0/16".parse().unwrap();
        wg_dev
            .set_ip(&"10.202.1.1".parse().unwrap(), &subnet)
            .unwrap();

        wg_dev.take_down_device().unwrap();

        let dev_present_final = wg_dev.check_device().unwrap();
        assert!(!dev_present_final);
    }

    #[test]
    fn test_create_device_with_ip_and_key() {
        let mut wg_dev = Arch::get_wg_dev("wgtest3");

        let _ = wg_dev.take_down_device();

        let dev_present_before = wg_dev.check_device().unwrap();
        assert!(!dev_present_before);

        wg_dev.create_device().unwrap();

        let dev_present_after = wg_dev.check_device().unwrap();
        assert!(dev_present_after);

        let subnet: ipnet::Ipv4Net = "10.203.0.0/16".parse().unwrap();
        wg_dev
            .set_ip(&"10.203.1.1".parse().unwrap(), &subnet)
            .unwrap();

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
