#[cfg(test)]
mod tests {
    use wg_netmanager::configuration::*;
    use wg_netmanager::unconnected::*;

    #[test]
    fn test_check_device_fail() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgx", "10.1.1.1");
        let dc = check_device(&sc);
        assert!(!dc);
    }

    #[test]
    fn test_bring_up_device() {
        let sc = StaticConfiguration::new(Verbosity::All, "wgy", "10.1.1.1");

        let dev_present_before = check_device(&sc);
        assert!(!dev_present_before);

        let dc = bring_up_device(&sc);

        let dev_present_after = check_device(&sc);
        assert!(dev_present_after);

        let dc1 = take_down_device(&sc);

        let dev_present_final = check_device(&sc);
        assert!(!dev_present_final);
    }
}
