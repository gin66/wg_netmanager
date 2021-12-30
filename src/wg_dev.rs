use std::process::{Command, Stdio};

use crate::configuration::*;

pub trait WireguardDevice {
    fn init(config: &StaticConfiguration) -> Self;
    fn check_device(&self) -> std::io::Result<bool>;
    fn bring_up_device(&self) -> std::io::Result<()>;
    fn take_down_device(&self) -> std::io::Result<()>;
}

pub struct WireguardDeviceLinux {
    verbosity: Verbosity,
    device_name: String,
}

impl WireguardDevice for WireguardDeviceLinux {
    fn init(config: &StaticConfiguration) -> Self {
        WireguardDeviceLinux {
            verbosity: config.verbosity,
            device_name: config.wg_name.clone()
        } 
    }
    fn check_device(&self) -> std::io::Result<bool> {
        if self.verbosity.info() {
            println!("Check for device");
        }

        let status = Command::new("ip")
            .arg("link")
            .arg("show")
            .arg(&self.device_name)
            .status()
            .unwrap();

        Ok(status.success())
    }
    fn bring_up_device(&self) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Bring up device");
        }

        let status = Command::new("sudo")
            .arg("ip")
            .arg("link")
            .arg("add")
            .arg(&self.device_name)
            .arg("type")
            .arg("wireguard")
            .status()
            .unwrap();

        if status.success() {
            println!("Interface {} created", self.device_name);
        }
        else {
        }
        Ok(())
    }
    fn take_down_device(&self) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Take down device");
        }

        let status = Command::new("sudo")
            .arg("ip")
            .arg("link")
            .arg("del")
            .arg(&self.device_name)
            .status()
            .unwrap();

        if status.success() {
            println!("Interface {} created", self.device_name);
        }
        else {
        }
        Ok(())
    }
}
