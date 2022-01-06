pub mod configuration;
pub mod crypt_udp;
pub mod error;
pub mod event;
pub mod manager;
pub mod tui_display;
pub mod util;
pub mod wg_dev;
pub mod wg_dev_linuxkernel;

#[cfg(target_os = "linux")]
pub mod interfaces;
