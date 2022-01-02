use std::io::Write;
use std::process::{Command, Stdio};

use crate::configuration::Verbosity;
use crate::wg_dev::WireguardDevice;

pub struct WireguardDeviceLinux {
    verbosity: Verbosity,
    device_name: String,
}
impl WireguardDeviceLinux {
    pub fn init<T: Into<String>>(wg_name: T, verbosity: Verbosity) -> Self {
        WireguardDeviceLinux {
            verbosity: verbosity,
            device_name: wg_name.into(),
        }
    }
    fn update_conf(&self, conf: &str, set_new: bool) -> Result<(), String> {
        let wg_cmd = if set_new { "setconf" } else { "syncconf" };

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

        let _result = cmd_tee.wait().unwrap();

        println!("temp file {}", fname);
        let mut cmd = Command::new("sudo")
            .arg("wg")
            .arg(wg_cmd)
            .arg(&self.device_name)
            .arg(&*fname)
            .spawn()
            .unwrap();
        let result = cmd.wait().unwrap();
        println!("wg {}: {:?}", wg_cmd, result);

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
}

impl WireguardDevice for WireguardDeviceLinux {
    fn check_device(&self) -> std::io::Result<bool> {
        if self.verbosity.info() {
            println!("Check for device {}", self.device_name);
        }

        let mut cmd = Command::new("ip")
            .arg("link")
            .arg("show")
            .arg(&self.device_name)
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        Ok(result.success())
    }
    fn bring_up_device(&self) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Bring up device");
        }

        let mut cmd = Command::new("sudo")
            .arg("ip")
            .arg("link")
            .arg("add")
            .arg(&self.device_name)
            .arg("type")
            .arg("wireguard")
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        if result.success() {
            println!("Interface {} created", self.device_name);
        } else {
        }

        let mut cmd = Command::new("sudo")
            .arg("ip")
            .arg("link")
            .arg("set")
            .arg(&self.device_name)
            .arg("up")
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        if result.success() {
            println!("Interface {} up", self.device_name);
        } else {
        }
        Ok(())
    }
    fn take_down_device(&self) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Take down device");
        }

        let mut cmd = Command::new("sudo")
            .arg("ip")
            .arg("link")
            .arg("del")
            .arg(&self.device_name)
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        if result.success() {
            println!("Interface {} destroyed", self.device_name);
        } else {
        }
        Ok(())
    }
    fn set_ip(&self, ip: &str) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Set IP {}", ip);
        }

        let mut cmd = Command::new("sudo")
            .arg("ip")
            .arg("addr")
            .arg("add")
            .arg(ip)
            .arg("dev")
            .arg(&self.device_name)
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        if result.success() {
            println!("Interface {} set ip", self.device_name);
        } else {
        }
        Ok(())
    }
    fn add_route(&self, route: &str) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Set route {}", route);
        }

        let mut cmd = Command::new("sudo")
            .arg("ip")
            .arg("route")
            .arg("add")
            .arg(route)
            .arg("dev")
            .arg(&self.device_name)
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        if result.success() {
            println!("Interface {} set route", self.device_name);
        } else {
        }
        Ok(())
    }
    fn del_route(&self, route: &str) -> std::io::Result<()> {
        if self.verbosity.info() {
            println!("Set route {}", route);
        }

        let mut cmd = Command::new("sudo")
            .arg("ip")
            .arg("route")
            .arg("del")
            .arg(route)
            .spawn()
            .unwrap();

        let result = cmd.wait().unwrap();

        if result.success() {
            println!("Interface {} set route", self.device_name);
        } else {
        }
        Ok(())
    }
    fn set_conf(&self, conf: &str) -> Result<(), String> {
        self.update_conf(conf, true)
    }
    fn sync_conf(&self, conf: &str) -> Result<(), String> {
        self.update_conf(conf, false)
    }
}
