pub mod configuration;
pub mod crypt_udp;
pub mod error;
pub mod event;
pub mod manager;
pub mod routedb;
pub mod node;
pub mod run_loop;
pub mod tui_display;
pub mod util;
pub mod wg_dev;

pub mod arch_def;
pub use arch_def::Architecture;

#[cfg(target_os = "linux")]
pub mod arch_linux;

#[cfg(target_os = "macos")]
pub mod arch_macos;

#[cfg(target_os = "windows")]
pub mod arch_windows;

#[cfg(target_os = "android")]
pub mod arch_android;

#[cfg(target_os = "linux")]
pub use crate::arch_linux::ArchitectureLinux as Arch;

#[cfg(target_os = "macos")]
pub use crate::arch_macos::ArchitectureMacOs as Arch;

#[cfg(target_os = "windows")]
pub use crate::arch_windows::ArchitectureWindows as Arch;

#[cfg(target_os = "android")]
pub use crate::arch_android::ArchitectureAndroid as Arch;
