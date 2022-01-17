use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use crate::crypt_udp::{AddressedTo, UdpPacket};
use crate::tui_display::TuiAppEvent;

#[derive(Debug)]
pub enum Event {
    Udp(UdpPacket, SocketAddr),
    UpdateWireguardConfiguration,
    CtrlC,
    SendAdvertisement {
        addressed_to: AddressedTo,
        to: SocketAddr,
        wg_ip: Ipv4Addr,
    },
    SendPingToAllDynamicPeers,
    SendRouteDatabaseRequest {
        to: SocketAddrV4,
    },
    SendRouteDatabase {
        to: SocketAddrV4,
    },
    SendLocalContactRequest {
        to: SocketAddrV4,
    },
    SendLocalContact {
        to: SocketAddrV4,
    },
    CheckAndRemoveDeadDynamicPeers,
    UpdateRoutes,
    TimerTick1s,
    TuiApp(TuiAppEvent),
    ReadWireguardConfiguration,
}
