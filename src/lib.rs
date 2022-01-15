pub mod configuration;
pub mod crypt_udp;
pub mod error;
pub mod event;
pub mod manager;
pub mod tui_display;
pub mod util;
pub mod wg_dev;
pub mod arch;
pub mod arch_linux;
pub mod main_loop;

#[cfg(target_os = "linux")]
pub mod interfaces;
