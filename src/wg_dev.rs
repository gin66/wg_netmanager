use std::io::Write;
use std::process::{Command, Stdio};

use crate::configuration::Verbosity;

pub trait WireguardDevice {
    fn init<T: Into<String>>(wg_name: T, verbosity: Verbosity) -> Self;
    fn check_device(&self) -> std::io::Result<bool>;
    fn bring_up_device(&self) -> std::io::Result<()>;
    fn take_down_device(&self) -> std::io::Result<()>;
    fn set_ip(&self, ip: &str) -> std::io::Result<()>;
    fn add_route(&self, route: &str) -> std::io::Result<()>;
    fn set_conf(&self, conf: &str) -> Result<(), String>;
    fn sync_conf(&self, conf: &str) -> Result<(), String>;
}

pub struct WireguardDeviceLinux {
    verbosity: Verbosity,
    device_name: String,
}

impl WireguardDevice for WireguardDeviceLinux {
    fn init<T: Into<String>>(wg_name: T, verbosity: Verbosity) -> Self {
        WireguardDeviceLinux {
            verbosity: verbosity,
            device_name: wg_name.into(),
        }
    }
    fn check_device(&self) -> std::io::Result<bool> {
        if self.verbosity.info() {
            println!("Check for device {}", self.device_name);
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
            println!("Interface {} up", self.device_name);
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
            println!("Interface {} set ip", self.device_name);
        } else {
        }
        Ok(())
    }
    fn add_route(&self, route: &str) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Set route {}", route);
        }

        let status = Command::new("sudo")
            .arg("ip")
            .arg("route")
            .arg("add")
            .arg(route)
            .arg("dev")
            .arg(&self.device_name)
            .status()
            .unwrap();

        if status.success() {
            println!("Interface {} set route", self.device_name);
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

        let mut cmd_tee = Command::new("sudo")
            .arg("tee")
            .arg("-a")
            .arg(&*fname)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("{:?}", e))?;

        write!(cmd_tee.stdin.as_ref().unwrap(), "{}", conf)
            .map_err(|e| format!("{:?}", e))
            .unwrap();

        let result = cmd_tee.wait().unwrap();

        println!("temp file {}", fname);
        let mut cmd = Command::new("sudo")
            .arg("wg")
            .arg("setconf")
            .arg(&self.device_name)
            .arg(&*fname)
            .spawn()
            .unwrap();
        let result = cmd.wait().unwrap();
        println!("wg setconf: {:?}", result);

        let _output = Command::new("sudo")
            .arg("rm")
            .arg(&*fname)
            .status()
            .unwrap();

        if result.success() {
            Ok(())
        } else {
            Err(format!("ERROR"))
        }
    }
    fn sync_conf(&self, conf: &str) -> Result<(), String> {
        let output = Command::new("sudo")
            .arg("mktemp")
            .arg("/tmp/wg_XXXXXXXXXX")
            .output()
            .unwrap();
        let tmpfname = String::from_utf8_lossy(&output.stdout);
        let fname = tmpfname.trim();

        let mut cmd_tee = Command::new("sudo")
            .arg("tee")
            .arg("-a")
            .arg(&*fname)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| format!("{:?}", e))?;

        write!(cmd_tee.stdin.as_ref().unwrap(), "{}", conf)
            .map_err(|e| format!("{:?}", e))
            .unwrap();

        let result = cmd_tee.wait().unwrap();

        println!("temp file {}", fname);
        let mut cmd = Command::new("sudo")
            .arg("wg")
            .arg("syncconf")
            .arg(&self.device_name)
            .arg(&*fname)
            .spawn()
            .unwrap();
        let result = cmd.wait_with_output().unwrap();
        println!("wg syncconf: {:?}", result);

        let _output = Command::new("sudo")
            .arg("rm")
            .arg(&*fname)
            .status()
            .unwrap();

        if result.status.success() {
            Ok(())
        } else {
            Err(format!("ERROR"))
        }
    }
}
