#[cfg(target_os = "linux")]
pub use crate::arch_linux::wg_dev_linuxkernel::WireguardDeviceLinux as ArchWireguardDevice;

#[cfg(target_os = "linux")]
pub use crate::arch_linux::get_local_interfaces;


#[cfg(target_os = "mac_os")]
pub use crate::arch_linux::wg_dev_linuxkernel::WireguardDeviceLinux as ArchWireguardDevice;

#[cfg(target_os = "mac_os")]
pub use crate::arch_linux::get_local_interfaces;
