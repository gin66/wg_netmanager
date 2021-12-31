use std::io::Write;
use std::process::{Command, Stdio};

use crate::configuration::*;

pub trait WireguardDevice {
    fn init(config: &StaticConfiguration) -> Self;
    fn check_device(&self) -> std::io::Result<bool>;
    fn bring_up_device(&self) -> std::io::Result<()>;
    fn take_down_device(&self) -> std::io::Result<()>;
    fn set_ip(&self, ip: &str) -> std::io::Result<()>;
    fn set_conf(&self, conf: &str) -> Result<(), String>;
}

pub struct WireguardDeviceLinux {
    verbosity: Verbosity,
    device_name: String,
}

impl WireguardDevice for WireguardDeviceLinux {
    fn init(config: &StaticConfiguration) -> Self {
        WireguardDeviceLinux {
            verbosity: config.verbosity,
            device_name: config.wg_name.clone(),
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
        } else {
        }

        let status2 = Command::new("sudo")
            .arg("ip")
            .arg("link")
            .arg("set")
            .arg(&self.device_name)
            .arg("up")
            .status()
            .unwrap();

        if status2.success() {
            println!("Interface {} created", self.device_name);
        } else {
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
            println!("Interface {} destroyed", self.device_name);
        } else {
        }
        Ok(())
    }
    fn set_ip(&self, ip: &str) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Set IP {}", ip);
        }

        let status = Command::new("sudo")
            .arg("ip")
            .arg("addr")
            .arg("add")
            .arg(ip)
            .arg("dev")
            .arg(&self.device_name)
            .status()
            .unwrap();

        if status.success() {
            println!("Interface {} created", self.device_name);
        } else {
        }
        Ok(())
    }
    fn set_conf(&self, conf: &str) -> Result<(), String> {
        let output = Command::new("sudo")
            .arg("mktemp")
            .arg("/tmp/wg_XXXXXXXXXX")
            .output()
            .unwrap();
        let tmpfname = String::from_utf8_lossy(&output.stdout);
        let fname = tmpfname.trim();

        let cmd_tee = Command::new("sudo")
            .arg("tee")
            .arg("-a")
            .arg(&*fname)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("{:?}", e))?;

        write!(cmd_tee.stdin.as_ref().unwrap(), "{}", conf).map_err(|e| format!("{:?}", e));

        println!("temp file {}", fname);
        let result = Command::new("sudo")
            .arg("wg")
            .arg("setconf")
            .arg(&self.device_name)
            .arg(&*fname)
            .output()
            .unwrap();
        println!("status {:?}", result);

        let output = Command::new("sudo")
            .arg("rm")
            .arg(&*fname)
            .status()
            .unwrap();

        if result.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&result.stderr).into_owned())
        }
    }
}
