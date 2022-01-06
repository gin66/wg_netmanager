use std::net::{SocketAddr, SocketAddrV4};

use crate::crypt_udp::UdpPacket;
use crate::tui_display::TuiAppEvent;

#[derive(Debug)]
pub enum Event {
    Udp(UdpPacket, SocketAddr),
    PeerListChange,
    CtrlC,
    SendAdvertisement { to: SocketAddr },
    SendAdvertisementToPublicPeers,
    SendPingToAllDynamicPeers,
    SendRouteDatabaseRequest { to: SocketAddrV4 },
    SendRouteDatabase { to: SocketAddrV4 },
    CheckAndRemoveDeadDynamicPeers,
    UpdateRoutes,
    TimerTick1s,
    TuiApp(TuiAppEvent),
}
