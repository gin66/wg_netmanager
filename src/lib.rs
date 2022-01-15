pub mod configuration;
pub mod crypt_udp;
pub mod error;
pub mod event;
pub mod main_loop;
pub mod manager;
pub mod tui_display;
pub mod util;
pub mod wg_dev;

pub mod arch_def;
pub use arch_def::Architecture;

#[cfg(target_os = "linux")]
pub mod arch_linux;

#[cfg(target_os = "mac_os")]
pub mod arch_macos;

#[cfg(target_os = "linux")]
pub use crate::arch_linux::ArchitectureLinux as Arch;

#[cfg(target_os = "mac_os")]
pub use crate::arch_macos::ArchitectureMacOs as Arch;
