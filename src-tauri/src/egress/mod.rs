//! Which network interface traffic would actually leave by.
//!
//! # Why this is not called `vpn`
//!
//! Flume cannot tell whether a VPN is running, and a module that claimed to
//! would be inventing a verdict the data cannot support — see architecture
//! rule 9. What it *can* establish, exactly and cheaply, is the interface the
//! kernel would use to reach the internet, and whether that interface is a
//! tunnel. "Traffic leaves through utun6, a tunnel interface" is checkable.
//! "You are protected by TorGuard" is a guess about a machine Flume cannot
//! see, and the UI never says it.
//!
//! # Why the route, and not the interface list
//!
//! The obvious check — enumerate interfaces, look for `utun`/`wg`/`tun` — is
//! wrong, and wrong in the dangerous direction. A macOS laptop with no VPN
//! connected routinely has half a dozen `utun` interfaces up and running, for
//! iCloud Private Relay, Continuity, Wi-Fi Calling and per-app VPN slots. The
//! development machine has nine, all `UP,POINTOPOINT,RUNNING`, while every
//! packet leaves through `en7`. A name scan reports "protected" there and is
//! believed.
//!
//! So the question asked here is the one that matters: *of the interfaces
//! present, which one would a packet to the internet actually use?*
//!
//! # No packets are sent
//!
//! [`source_address`] binds a UDP socket and calls `connect`. On every
//! platform Flume targets that is a local operation — a route lookup and a
//! bind — and no datagram is transmitted. The address it returns is the source
//! address a peer would see. This is deliberately not an "what is my IP"
//! request to a web service: architecture rule 10 puts every byte of egress in
//! `usage::sender`, and a privacy feature that phoned home to work would be a
//! poor joke.
//!
//! # The MAC is a signal on two platforms out of three
//!
//! A point-to-point tunnel has no link-layer address, so "no MAC" looks like
//! free corroboration. Measured through this crate it is worth exactly what
//! the platform's code path is worth, and the three paths are not equal:
//!
//! * **Windows** — correct. `make_mac_address` returns `None` when
//!   `PhysicalAddressLength` is 0, and TorGuard's `wg-torguard` adapter
//!   reports no physical address while `Ethernet` reports
//!   `00-1C-42-A6-85-8B`.
//! * **Linux** — correct by construction: the address comes from an
//!   `AF_PACKET` entry's `sockaddr_ll`, and a tunnel has no such entry.
//! * **macOS** — useless. It reports `en7`, which `ifconfig` gives as
//!   `80:6d:97:63:a2:67`, as `02:00:00:00:00:00`, and every `utun` and `lo0`
//!   as `00:00:00:00:00:00` — a placeholder read from what looks like the
//!   wrong offset in `sockaddr_dl`.
//!
//! That asymmetry is survivable because of *which* direction each path fails
//! in. macOS never returns `None`, so a rule of the form "no MAC suggests a
//! tunnel" simply never fires there — inert rather than wrong — and macOS
//! names its own tunnels `utun`, so the name carries it. Read an absent MAC
//! as evidence *for* a tunnel, never a present one as evidence *against*,
//! and all three platforms stay honest.
//!
//! # The name is not equally trustworthy either
//!
//! macOS names tunnel interfaces itself — `utun6` — and that prefix is the
//! operating system's. Everywhere WireGuard is involved the name belongs to
//! whoever wrote the config, and TorGuard measurably does not write the same
//! one twice:
//!
//! | Platform | Interface     |
//! | -------- | ------------- |
//! | Windows  | `wg-torguard` |
//! | Linux    | `torguard-wg` |
//!
//! One vendor, two platforms, the components reversed, and neither is `wg0`.
//! A `starts_with("wg")` rule catches the first and misses the second, and
//! nothing about the second is unusual — `wg-quick` names the interface after
//! the config file on Linux exactly as the Windows client does.
//!
//! This is what makes the MAC load-bearing rather than decorative: on the
//! Linux box `torguard-wg` reports no address at all while `eth0` reports
//! `00:15:5d:7f:f7:07`, so the tunnel is identifiable when its name is not.
//!
//! Its OpenVPN adapter is the hard case. It is a TAP-Windows Adapter V9 whose
//! friendly name is `Local Area Connection` — sitting in the list beside
//! `Local Area Connection* 6`, a WAN Miniport that is not a tunnel — it *has*
//! a MAC (`00-FF-FD-61-9F-3D`), and its media type is 802.3. Neither the name
//! nor the MAC separates it from a real Ethernet adapter. The one field that
//! does is `IP_ADAPTER_ADDRESSES.IfType`: 53 (`propVirtual`) for both TorGuard
//! adapters against 6 (`ethernetCsmacd`) for Ethernet. This crate reads that
//! field — `windows.rs` derives `internal` from it — and does not expose it,
//! and Flume forbids `unsafe_code`, so there is no route to it from here.
//!
//! The consequence is deliberate: with TorGuard in OpenVPN mode on Windows the
//! guard will not recognise the tunnel. That is a false negative — it holds
//! transfer that was in fact protected, the user notices at once, and
//! [`Verdict::Pinned`] is the way out. Widening the name match to catch
//! `Local Area Connection` would trade it for calling a WAN Miniport a
//! tunnel, which is the failure that actually costs someone something.
//!
//! How far that reaches is worth stating, because it bounds the exposure. On
//! the Windows 11 **ARM** machine this was measured on, WireGuard is the only
//! one of TorGuard's three tunnel types that connects at all — OpenVPN and
//! OpenConnect both fail. The probable reason is that Windows on ARM does not
//! emulate kernel-mode drivers, so an x64 `.sys` cannot load, and WireGuard's
//! Wintun ships ARM64 while the TAP-family drivers effectively do not. That
//! cause is inferred and not verified; the observation is not.
//!
//! So the gap is one protocol on x64 Windows, where WireGuard — the mode that
//! *is* recognised — is also the default TorGuard offers. It is not nothing,
//! and it is not enough to justify taking a new dependency for
//! `IP_ADAPTER_ADDRESSES.IfType` against this project's preference for crates
//! already in the tree. Revisit if someone reports it, or if a second VPN
//! client turns out to present the same way.
//!
//! # Measured behaviour worth keeping
//!
//! **macOS renumbers on every connect.** The development machine went from
//! `utun0`–`utun8` to `utun0`–`utun11` to `utun0`–`utun12` across two TorGuard
//! connect cycles in one afternoon, and the old interfaces persist after
//! disconnect. A pin naming `utun12` is wrong the next time the VPN
//! reconnects, which is why the default accepts any tunnel and the settings
//! copy warns about pinning on macOS specifically. Windows and Linux name the
//! adapter after the config and are stable.
//!
//! **`other_family_outside` has never been observed firing.** TorGuard removes
//! IPv6 entirely rather than leaving it outside the tunnel — measured on Linux
//! with WireGuard and macOS with OpenConnect, both of which report no IPv6
//! route at all while connected, against an `en7`/`eth0` that carries a global
//! v6 address when disconnected. The flag is still right to compute and report
//! — a v4-only tunnel beside a live v6 default route is a real configuration —
//! but no UI copy written for it has been checked against a machine actually
//! in that state.
//!
//! **All three TorGuard tunnel types land on `utun` on macOS.** OpenConnect is
//! measured (`utun12`); WireGuard is measured. macOS gives unprivileged
//! tunnels no other option since kexts were blocked on Apple Silicon, so the
//! `utun` prefix covers the protocol choice rather than any one protocol.
//!
//! # What this cannot tell apart
//!
//! A point-to-point link with no link-layer address is what a VPN tunnel looks
//! like from here, and it is also what a USB cellular modem and a direct PPPoE
//! DSL connection look like. Both present as `ppp0` with no MAC, and both are
//! ordinary internet connections carrying no privacy at all. `ppp` is
//! deliberately kept out of the tunnel *name* list for that reason, but the
//! MAC rule reaches them anyway, so someone whose machine dials PPPoE directly
//! rather than through a router would be told they are on a tunnel.
//!
//! There is no signal here that separates the two: they are the same kind of
//! link, differing only in where the far end is, which is exactly what Flume
//! cannot see. `IP_ADAPTER_ADDRESSES.IfType` distinguishes them on Windows
//! (23, `ppp`) and is not exposed; Linux offers no equivalent through this
//! crate. Recorded rather than papered over — it is the one false positive
//! left in the classifier, and a user on PPPoE should know the guard is
//! telling them something it cannot actually establish.
//!
//! Like [`crate::engine`], this module imports no Tauri types and runs under a
//! plain `cargo test`.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use serde::{Deserialize, Serialize};

/// Destination used to provoke an IPv4 route lookup.
///
/// TEST-NET-1 from RFC 5737, reserved for documentation and guaranteed never
/// to be a real host. It follows the default route like any other public
/// address, which is the whole point — peer traffic goes to arbitrary internet
/// addresses, so the default route is what decides where it leaves from.
/// Nothing is sent to it.
///
/// Checked rather than assumed, since a reserved range is plausibly the sort
/// of thing a VPN's leak protection blackholes. With TorGuard connected on
/// Windows, `192.0.2.1`, `1.1.1.1` and `8.8.8.8` all resolve to the tunnel:
/// WireGuard covers the space with four `/2` routes that beat its own `/1`
/// blackholes, and no reserved prefix is singled out.
const PROBE_V4: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9);

/// Destination used to provoke an IPv6 route lookup.
///
/// `2001:db8::/32` from RFC 3849, the IPv6 documentation prefix. Same
/// reasoning as [`PROBE_V4`]; nothing is sent.
const PROBE_V6: SocketAddr = SocketAddr::new(
    IpAddr::V6(Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1)),
    9,
);

/// What Flume does when traffic is not leaving through an acceptable tunnel.
///
/// Three states rather than a bool, and the middle one is the point. A user
/// who wants to *know* is not the same as a user who wants transfer stopped,
/// and a client that only offered the second would be answered by people
/// switching the whole thing off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EgressGuard {
    /// Do not check.
    ///
    /// The default. See [`crate::settings::Settings::egress_guard`] for why a
    /// general-purpose client does not warn about this uninvited.
    #[default]
    Off,

    /// Check and say so, but never stop anything.
    Warn,

    /// Check, and hold all transfer while the answer is no.
    ///
    /// Held, not paused: the distinction is load-bearing everywhere
    /// downstream. A torrent the user paused stays paused when the tunnel
    /// comes back, and a torrent the guard held resumes. Collapsing the two
    /// either resumes something the user stopped by hand, or strands
    /// everything until they notice.
    Hold,
}

impl EgressGuard {
    /// Whether the check runs at all.
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Whether a failed check should stop transfer.
    #[must_use]
    pub const fn holds_transfer(self) -> bool {
        matches!(self, Self::Hold)
    }
}

/// What kind of thing an interface is.
///
/// Deliberately three-valued. `Unknown` is not a polite way of saying
/// "probably fine" — it is the statement that the evidence did not settle it,
/// and the guard treats it as *not* a tunnel for exactly that reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InterfaceKind {
    /// A tunnel: WireGuard, OpenVPN, IPsec, or similar.
    Tunnel,
    /// An ordinary adapter — Ethernet, Wi-Fi, cellular.
    Ordinary,
    /// The evidence did not settle it.
    Unknown,
}

/// The interface one address family leaves by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hop {
    /// The interface name, as the operating system gives it.
    ///
    /// `utun6` on macOS, `wg0` or `tun0` on Linux, and on Windows the adapter's
    /// *friendly name* — which is user-editable, so it can be anything at all.
    /// That is why it is described rather than quoted in a diagnostics bundle;
    /// see [`crate::diagnostics`].
    pub interface: String,

    /// What kind of interface that is.
    pub kind: InterfaceKind,
}

/// Where each address family would leave from.
///
/// Either half may be `None`: a network with no working IPv6 has no IPv6
/// route, which is ordinary and not a fault.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressPath {
    /// Where IPv4 leaves from.
    pub v4: Option<Hop>,
    /// Where IPv6 leaves from.
    pub v6: Option<Hop>,
}

/// Whether transfer should be allowed, and why not if not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// `rename_all_fields` is not optional, for the same reason `usage::EventKind`
// carries it: `rename_all` renames the *variants* only, so without it
// `other_family_outside` goes to the frontend under its snake_case name while
// the TypeScript mirror reads `otherFamilyOutside` and quietly sees
// `undefined` — a leak warning that never fires.
#[serde(
    tag = "verdict",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Verdict {
    /// Traffic leaves through a tunnel, and through the pinned one if any was
    /// named.
    Tunnelled {
        /// The interface it leaves by.
        interface: String,
        /// Whether the *other* address family leaves outside that tunnel.
        ///
        /// A tunnel that carries IPv4 while IPv6 goes out raw is the classic
        /// leak, and it is reported rather than folded into the verdict — a
        /// machine with a working IPv6 default route beside a v4-only tunnel
        /// is still doing what the user asked for over v4.
        other_family_outside: bool,
    },

    /// Traffic leaves through the interface the user pinned, which Flume
    /// could not itself identify as a tunnel.
    ///
    /// Transfer is allowed: the user naming an interface outranks a name
    /// heuristic, and without this the pin would be unable to rescue anything
    /// the classifier failed on — which is precisely what it is for. TorGuard
    /// in OpenVPN mode on Windows presents as `Local Area Connection` with a
    /// MAC and an 802.3 media type, indistinguishable from Ethernet through
    /// what this crate exposes, and no amount of classifier work reaches it.
    ///
    /// Kept separate from [`Self::Tunnelled`] so the UI never claims more than
    /// it knows. "Traffic leaves through the interface you pinned" is true;
    /// "traffic leaves through a tunnel" would be the user's assertion
    /// repeated back to them as Flume's finding.
    Pinned {
        /// The interface it leaves by, which the user named.
        interface: String,
        /// Whether the other address family leaves outside that interface.
        other_family_outside: bool,
    },

    /// Traffic leaves through an ordinary adapter.
    Direct {
        /// The interface it leaves by.
        interface: String,
    },

    /// Traffic leaves through a tunnel, but not the one that was pinned.
    ///
    /// Distinct from [`Self::Direct`] on purpose: the fix is different. This
    /// one is usually a VPN client that reconnected onto a fresh interface
    /// number, and the remedy is to re-pin, not to connect anything.
    WrongTunnel {
        /// The interface it actually leaves by.
        interface: String,
        /// The interface that was pinned in settings.
        expected: String,
    },

    /// There is no route to the internet, or the interface holding it could
    /// not be identified.
    ///
    /// Reported as itself rather than resolved towards either answer. A laptop
    /// with the lid shut and a machine behind an interface Flume failed to
    /// enumerate are both here, and neither is evidence of a tunnel.
    Unknown,
}

impl Verdict {
    /// Whether this verdict permits transfer.
    ///
    /// [`Verdict::Unknown`] does not. A guard the user switched on to hold
    /// traffic must fail closed, or it is decoration.
    #[must_use]
    pub const fn allows_transfer(&self) -> bool {
        matches!(self, Self::Tunnelled { .. } | Self::Pinned { .. })
    }
}

/// The source address the kernel would use to reach `destination`.
///
/// Sends nothing. `connect` on a UDP socket performs a route lookup and binds
/// a local address; the datagram that would follow is never written.
///
/// Returns `None` when there is no route for that address family, which is the
/// ordinary answer for IPv6 on a v4-only network.
#[must_use]
pub fn source_address(destination: SocketAddr) -> Option<IpAddr> {
    let bind: SocketAddr = if destination.is_ipv4() {
        (Ipv4Addr::UNSPECIFIED, 0).into()
    } else {
        (Ipv6Addr::UNSPECIFIED, 0).into()
    };

    let socket = UdpSocket::bind(bind).ok()?;
    socket.connect(destination).ok()?;
    let local = socket.local_addr().ok()?.ip();

    // A bound-but-unrouted socket reports the unspecified address. That is not
    // an answer, and returning it would have the caller looking for an
    // interface that owns 0.0.0.0.
    if local.is_unspecified() {
        return None;
    }
    Some(local)
}

/// Decides what kind of interface this is, from everything the platform says
/// about it.
///
/// **This is the judgement call at the centre of the feature**, and the place
/// a wrong answer does real harm in both directions: calling an ordinary
/// adapter a tunnel tells someone they are covered when they are not, and
/// calling a tunnel ordinary holds a download that should be running.
///
/// The evidence available, for the caller and for whoever revises this:
///
/// * `name` — exactly as the platform gives it. `utun6` on macOS,
///   `wg0`/`tun0`/`ppp0` on Linux, and on Windows the adapter's *friendly
///   name*, which the user can rename to anything and whose case is not
///   guaranteed. Measured examples: `wg-torguard`, `Local Area Connection`,
///   `Ethernet`, `en7`, `lo0`.
/// * `mac_addr` — absent for a tunnel on Windows and Linux, and *never*
///   absent on macOS, where this crate reports a placeholder for every
///   interface. Usable in one direction only: absence is evidence for a
///   tunnel, presence is not evidence against one. The module docs carry the
///   measurements.
/// * `internal` — the platform's own "loopback or otherwise not remotely
///   reachable" flag. Correct everywhere checked, and the only thing
///   separating `lo0` from a tunnel.
///
/// Returning [`InterfaceKind::Unknown`] is a legitimate answer and is treated
/// as "not a tunnel" downstream, so it is the safe thing to return when the
/// evidence genuinely does not settle it.
#[must_use]
pub fn classify(name: &str, mac_addr: Option<&str>, internal: bool) -> InterfaceKind {
    // Loopback first, and it must not reach the MAC rule below. Windows
    // loopback reports no physical address at all, so "absent means tunnel"
    // would call it a tunnel -- and loopback is exactly where traffic lands
    // while WireGuard's /1 blackholes are in force and its /2 routes are not.
    // That is a VPN *dropping*, and reading it as a tunnel would allow
    // transfer at the one moment the guard exists to stop it.
    if internal {
        return InterfaceKind::Unknown;
    }

    let name = name.to_ascii_lowercase();

    // Names the operating system assigns to tunnel devices. macOS is carried
    // entirely by this: it names every tunnel `utun`, and its MAC is a
    // placeholder that never reads as absent.
    //
    // `ppp` is deliberately absent. It is a point-to-point link, not
    // necessarily a private one -- a USB cellular modem and a direct PPPoE
    // DSL connection are both `ppp0` and both perfectly ordinary. The MAC
    // rule below reaches it anyway, which is a limitation rather than a save;
    // see the module docs.
    // Tunnels in the networking sense that carry no privacy whatever. 6to4,
    // IPIP and GRE are transition and routing mechanisms, plaintext to
    // everyone on the path -- so calling one a tunnel would tell a user they
    // are protected at a moment when they are measurably not, which is the
    // expensive direction. Checked before the tunnel prefixes so that `tunl0`
    // (IPIP) does not match on `tun`.
    //
    // macOS hides this by accident: `gif0` and `stf0` report a placeholder MAC
    // rather than none, so they never reach the MAC rule. Linux's `sit0` and
    // `gre0` report a genuine `None` and would otherwise read as tunnels.
    const PLAINTEXT_TUNNEL_PREFIXES: [&str; 6] = ["gif", "stf", "sit", "gre", "tunl", "ip6tnl"];
    if PLAINTEXT_TUNNEL_PREFIXES
        .iter()
        .any(|p| name.starts_with(p))
    {
        return InterfaceKind::Unknown;
    }

    const TUNNEL_PREFIXES: [&str; 5] = ["utun", "tun", "tap", "wg", "ipsec"];
    if TUNNEL_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return InterfaceKind::Tunnel;
    }

    // Ordinary adapters, checked before the MAC rule rather than after it.
    // Windows' "Ethernet (Kernel Debugger)" reports no physical address while
    // being an `ethernetCsmacd` interface, so the MAC rule alone would call it
    // a tunnel.
    const ORDINARY_PREFIXES: [&str; 12] = [
        // macOS: en0..en8, plus Apple's own link types.
        "en", "bridge", "vmenet", "awdl", "llw", "anpi", "nan", "ap",
        // Linux: eth0, enp3s0, wlan0, wlp2s0.
        "eth", "wlan", "wlp", // Windows, whose friendly names are words.
        "wi-fi",
    ];
    if ORDINARY_PREFIXES.iter().any(|p| name.starts_with(p)) {
        return InterfaceKind::Ordinary;
    }

    // Evidence for a tunnel, never against one. This is what identifies
    // `torguard-wg`, which no name rule reaches, and any WireGuard adapter
    // named after a config file rather than after a convention.
    if mac_addr.is_none() {
        return InterfaceKind::Tunnel;
    }

    // A name that matched nothing and an address that says nothing. The guard
    // treats this as "not a tunnel" and the UI says Flume could not tell,
    // which is the true statement -- claiming an ordinary adapter here would
    // be the same invention in the opposite direction.
    InterfaceKind::Unknown
}

/// Finds the interface holding `address`, and says what kind it is.
///
/// Returns `None` when no enumerated interface claims the address, which
/// happens if the routing table changed between the probe and the walk.
#[must_use]
fn hop_for(address: IpAddr, interfaces: &[NetworkInterface]) -> Option<Hop> {
    let interface = interfaces
        .iter()
        .find(|candidate| candidate.addr.iter().any(|addr| addr.ip() == address))?;

    Some(Hop {
        kind: classify(
            &interface.name,
            interface.mac_addr.as_deref(),
            interface.internal,
        ),
        interface: interface.name.clone(),
    })
}

/// How long the verdict must permit continuously before transfer is released.
///
/// Asymmetric on purpose, and the asymmetry is the design: a failing verdict
/// takes effect on the tick it appears, while a permitting one has to hold for
/// this long. Protection is never delayed; only recovery is.
///
/// Ten seconds is chosen against what actually flaps. A laptop waking, a VPN
/// reconnecting onto a new interface and a Wi-Fi handover all resolve in a few
/// seconds, and releasing into the middle of one costs a full re-announce to
/// every tracker plus a DHT announce, per torrent — slow for the user and rude
/// to the swarm. Ten seconds is long enough to sit out those transients and
/// short enough that a genuine reconnect is not noticed as downtime.
pub const SETTLE: Duration = Duration::from_secs(10);

/// Whether transfer may happen, given a history of verdicts over time.
///
/// Pure and clock-injected: [`Self::observe`] takes the current instant rather
/// than reading one, so the hysteresis can be tested without sleeping.
#[derive(Debug, Clone)]
pub struct TransferGate {
    settle: Duration,
    state: GateState,
}

/// Where the gate currently sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateState {
    /// Transfer is permitted and running.
    Open,
    /// Held, with the verdict still failing.
    Held,
    /// Held, but the verdict has permitted continuously since this instant.
    Settling {
        /// When the verdict started permitting.
        since: Instant,
    },
}

/// What the gate says to do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// Transfer may proceed.
    Open,
    /// Transfer must not proceed.
    Held,
}

impl Gate {
    /// Whether transfer may proceed.
    #[must_use]
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }
}

impl Default for TransferGate {
    fn default() -> Self {
        Self::new(SETTLE)
    }
}

impl TransferGate {
    /// A gate that releases after `settle` of continuous permission.
    ///
    /// Starts [`Gate::Held`]. A gate that began open would permit transfer for
    /// the first tick of its life, which is exactly the launch window this
    /// feature exists to close.
    #[must_use]
    pub const fn new(settle: Duration) -> Self {
        Self {
            settle,
            state: GateState::Held,
        }
    }

    /// Folds one verdict into the gate and returns what to do.
    pub fn observe(&mut self, permitted: bool, now: Instant) -> Gate {
        if !permitted {
            // Immediate, from any state. A verdict that stopped permitting
            // while the settle window was running resets it rather than
            // shortening it.
            self.state = GateState::Held;
            return Gate::Held;
        }

        self.state = match self.state {
            GateState::Open => GateState::Open,
            GateState::Held => GateState::Settling { since: now },
            GateState::Settling { since } => {
                if now.duration_since(since) >= self.settle {
                    GateState::Open
                } else {
                    GateState::Settling { since }
                }
            }
        };

        match self.state {
            GateState::Open => Gate::Open,
            GateState::Held | GateState::Settling { .. } => Gate::Held,
        }
    }

    /// What the gate last decided, without advancing it.
    #[must_use]
    pub const fn gate(&self) -> Gate {
        match self.state {
            GateState::Open => Gate::Open,
            GateState::Held | GateState::Settling { .. } => Gate::Held,
        }
    }

    /// How much of the settle window remains, if one is running.
    ///
    /// For the UI, which owes the user a reason rather than an unexplained
    /// wait: "a tunnel is back; transfer resumes in 6 s" is a status, "held"
    /// is not.
    #[must_use]
    pub fn settling_for(&self, now: Instant) -> Option<Duration> {
        match self.state {
            GateState::Settling { since } => {
                Some(self.settle.saturating_sub(now.duration_since(since)))
            }
            GateState::Open | GateState::Held => None,
        }
    }

    /// Drops the settle window and opens immediately.
    ///
    /// For changes the *user* just made — switching the guard off, or editing
    /// the pinned interface. Someone who has just retyped an interface name is
    /// watching the window, and making them wait ten seconds to find out
    /// whether they got it right turns a settling period into a bug report.
    pub const fn release_now(&mut self) {
        self.state = GateState::Open;
    }

    /// Returns to held and forgets any settle progress.
    ///
    /// For a policy change that could only make things stricter — switching
    /// *into* Hold, or pinning an interface — where carrying over a window
    /// that started under the old policy would let transfer through on the
    /// strength of a verdict nobody judged against the new one.
    pub const fn hold_now(&mut self) {
        self.state = GateState::Held;
    }
}

/// Everything the UI needs to explain the guard, and the engine loop needs to
/// act on it.
///
/// Published once per tick by the guard loop, which is the *only* thing that
/// probes. A second prober would read the routing table at a different instant
/// and disagree with the first, and a guard that contradicts itself on screen
/// is worse than no guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardStatus {
    /// The mode the user chose.
    pub guard: EgressGuard,
    /// Where traffic leaves, and what Flume makes of it.
    pub report: EgressReport,
    /// Whether transfer is being held right now.
    ///
    /// Always `false` unless the guard is [`EgressGuard::Hold`]: `Warn` says
    /// so and stops nothing.
    pub held: bool,
    /// Seconds until transfer resumes, while a settle window is running.
    ///
    /// `None` when transfer is running, and when it is held with no prospect
    /// of resuming. The UI owes the user the difference between "held, and
    /// counting down" and "held, and waiting for you" — an unexplained pause
    /// is the thing that gets a privacy feature switched off.
    pub resumes_in_seconds: Option<u64>,
}

/// Repeated probing without repeated interface enumeration.
///
/// # Why this exists
///
/// The two halves of [`probe`] cost wildly different amounts. Measured on the
/// development Mac, in release, over fifty rounds:
///
/// | Step                     | Cost      |
/// | ------------------------ | --------- |
/// | one route lookup         | 27.7 µs   |
/// | one interface enumeration| 3223.3 µs |
///
/// The enumeration is 116x the lookup and 99% of the total, which puts a naive
/// probe at 3.3 ms — a third of the whole 1 Hz telemetry budget, for an answer
/// that changes when someone connects a VPN and at no other time.
///
/// So the route lookup runs every time and the enumeration does not. If the
/// source address for a family is the one seen last time, the interface that
/// owns it is the one seen last time, and the cached [`Hop`] still stands.
///
/// That assumption is worth stating plainly: an address could in principle be
/// moved between interfaces without changing, in a failover arrangement that
/// keeps the address and swaps the hardware under it. Flume would keep the
/// stale interface name until the address next changed. The alternative is
/// paying 3.2 ms every second on every machine to cover a case that does not
/// arise on a laptop, which is the wrong trade.
///
/// Note what this is *not*: it is not a time-based cache, so there is no
/// staleness window. A VPN dropping changes the source address, which misses
/// the cache, which re-enumerates on that very tick.
#[derive(Debug, Default)]
pub struct EgressWatcher {
    /// The IPv4 source address last seen, and the hop it resolved to.
    v4: Option<(IpAddr, Hop)>,
    /// The IPv6 source address last seen, and the hop it resolved to.
    v6: Option<(IpAddr, Hop)>,
}

impl EgressWatcher {
    /// Reads the current path, enumerating interfaces only if an address moved.
    pub fn path(&mut self) -> EgressPath {
        self.resolve(
            source_address(PROBE_V4),
            source_address(PROBE_V6),
            interfaces,
        )
    }

    /// Reads the current path and judges it against `pinned`.
    pub fn report(&mut self, pinned: Option<&str>) -> EgressReport {
        let path = self.path();
        let verdict = path.verdict(pinned);
        EgressReport { path, verdict }
    }

    /// The cache decision, separated from the syscalls so it can be tested.
    ///
    /// `enumerate` is called at most once per invocation even when both
    /// families miss, because one walk of the interface list answers both.
    fn resolve<F>(
        &mut self,
        v4_source: Option<IpAddr>,
        v6_source: Option<IpAddr>,
        enumerate: F,
    ) -> EgressPath
    where
        F: FnOnce() -> Vec<NetworkInterface>,
    {
        /// Whether this family needs the interface list to answer.
        fn stale(source: Option<IpAddr>, cached: Option<&(IpAddr, Hop)>) -> bool {
            match (source, cached) {
                // No route: nothing to resolve, and nothing to enumerate for.
                (None, _) => false,
                (Some(source), Some((seen, _))) => source != *seen,
                (Some(_), None) => true,
            }
        }

        if stale(v4_source, self.v4.as_ref()) || stale(v6_source, self.v6.as_ref()) {
            let interfaces = enumerate();
            for (source, slot) in [(v4_source, &mut self.v4), (v6_source, &mut self.v6)] {
                if let Some(source) = source
                    && slot.as_ref().is_none_or(|(seen, _)| *seen != source)
                {
                    *slot = hop_for(source, &interfaces).map(|hop| (source, hop));
                }
            }
        }

        // A family that lost its route loses its cache entry too, or the path
        // would keep reporting an interface nothing is leaving by.
        if v4_source.is_none() {
            self.v4 = None;
        }
        if v6_source.is_none() {
            self.v6 = None;
        }

        EgressPath {
            v4: self.v4.as_ref().map(|(_, hop)| hop.clone()),
            v6: self.v6.as_ref().map(|(_, hop)| hop.clone()),
        }
    }
}

/// The current egress path and what Flume makes of it.
///
/// Both halves cross the IPC boundary together because the verdict is derived
/// from the path *and* the user's pin, and deriving it in the frontend would
/// put the decision in two places — the one thing architecture rule 3 exists to
/// prevent. The path travels alongside it so the UI can name the interface
/// without re-deriving anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EgressReport {
    /// Where each address family leaves from.
    pub path: EgressPath,
    /// Whether that permits transfer, and why not if not.
    pub verdict: Verdict,
}

impl EgressReport {
    /// Probes the machine and judges the result against `pinned`.
    #[must_use]
    pub fn current(pinned: Option<&str>) -> Self {
        let path = probe();
        let verdict = path.verdict(pinned);
        Self { path, verdict }
    }
}

/// Every interface the platform reports.
///
/// Exposed so callers that probe repeatedly can measure and cache it
/// separately: this is the expensive half of [`probe`] by a wide margin, and
/// unlike a route lookup its answer changes only when an interface appears or
/// disappears.
#[must_use]
pub fn interfaces() -> Vec<NetworkInterface> {
    NetworkInterface::show().unwrap_or_default()
}

/// Every interface on the machine, classified and ordered for a picker.
///
/// Exists because the interface pin is otherwise unusable. A user whose pin has
/// gone stale — which on macOS happens on every VPN reconnect, since each one
/// lands on a fresh `utun` — has a held library and a text field, and the
/// remedy is retyping a name the application already knows and will not show
/// them.
///
/// Ordered tunnels first, then ordinary adapters, then whatever could not be
/// classified, alphabetically within each group. Someone opening this list is
/// looking for their VPN, so it is at the top.
///
/// Loopback is excluded. Pinning it would hold transfer permanently, which is
/// not a choice worth offering.
#[must_use]
pub fn candidates() -> Vec<Hop> {
    /// Sort key: tunnels first, then ordinary, then unclassifiable.
    const fn rank(kind: InterfaceKind) -> u8 {
        match kind {
            InterfaceKind::Tunnel => 0,
            InterfaceKind::Ordinary => 1,
            InterfaceKind::Unknown => 2,
        }
    }

    let mut hops: Vec<Hop> = interfaces()
        .into_iter()
        .filter(|interface| !interface.internal)
        .map(|interface| Hop {
            kind: classify(
                &interface.name,
                interface.mac_addr.as_deref(),
                interface.internal,
            ),
            interface: interface.name,
        })
        .collect();

    // The platform can list one interface once per address family; the picker
    // wants it once.
    hops.sort_by(|a, b| {
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then_with(|| a.interface.cmp(&b.interface))
    });
    hops.dedup_by(|a, b| a.interface == b.interface);
    hops
}

/// Reads the current egress path for both address families.
///
/// Two route lookups and one interface enumeration. Nothing is sent and
/// nothing is cached — the caller decides how often this is worth asking, and
/// [`crate::telemetry`] does not ask every tick.
#[must_use]
pub fn probe() -> EgressPath {
    let interfaces = interfaces();

    EgressPath {
        v4: source_address(PROBE_V4).and_then(|ip| hop_for(ip, &interfaces)),
        v6: source_address(PROBE_V6).and_then(|ip| hop_for(ip, &interfaces)),
    }
}

impl EgressPath {
    /// Judges this path against the user's policy.
    ///
    /// `pinned` names the one interface the user will accept, or `None` to
    /// accept any tunnel.
    ///
    /// # Which family decides
    ///
    /// IPv4, when there is an IPv4 route. Peer traffic is overwhelmingly v4,
    /// and a v4-only tunnel beside a working v6 default route is a real
    /// situation that the user is entitled to be told about rather than
    /// blocked over — so the v6 leak rides along on the verdict as
    /// `other_family_outside` instead of overriding it.
    ///
    /// On a v6-only network there is no v4 route to judge, and falling back to
    /// v6 is the only reading that is not simply wrong; refusing to judge
    /// there would hold every transfer on a network that is working perfectly.
    #[must_use]
    pub fn verdict(&self, pinned: Option<&str>) -> Verdict {
        let (deciding, other) = match (self.v4.as_ref(), self.v6.as_ref()) {
            (Some(v4), other) => (v4, other),
            (None, Some(v6)) => (v6, None),
            (None, None) => return Verdict::Unknown,
        };

        let other_family_outside = other.is_some_and(|hop| hop.interface != deciding.interface);

        // The pin is answered before the classifier, because it outranks it.
        // A user who names an interface is asserting something the classifier
        // cannot see -- and on Windows in OpenVPN mode there is nothing for
        // the classifier to see, so a pin that could not override it would be
        // a setting that rescues nothing.
        if let Some(expected) = pinned {
            // Compared case-insensitively and trimmed, because this string is
            // typed by a person while the other side comes from the operating
            // system. Someone who types `ethernet` where Windows says
            // `Ethernet`, or pastes a name with a trailing space, would
            // otherwise get `WrongTunnel` forever — a permanently held library
            // whose cause is invisible, since the two names look identical
            // wherever they are displayed.
            if !expected
                .trim()
                .eq_ignore_ascii_case(deciding.interface.trim())
            {
                return Verdict::WrongTunnel {
                    interface: deciding.interface.clone(),
                    expected: expected.to_owned(),
                };
            }
            return if deciding.kind == InterfaceKind::Tunnel {
                Verdict::Tunnelled {
                    interface: deciding.interface.clone(),
                    other_family_outside,
                }
            } else {
                // Allowed, but reported as the user's assertion rather than
                // as Flume's finding. Rule 9: the stronger claim is not
                // available, so the stronger claim is not made.
                Verdict::Pinned {
                    interface: deciding.interface.clone(),
                    other_family_outside,
                }
            };
        }

        match deciding.kind {
            InterfaceKind::Tunnel => Verdict::Tunnelled {
                interface: deciding.interface.clone(),
                other_family_outside,
            },
            InterfaceKind::Ordinary => Verdict::Direct {
                interface: deciding.interface.clone(),
            },
            // An interface Flume could not classify is not a tunnel, and
            // saying "direct" about it would be a claim of the same
            // confidence in the other direction.
            InterfaceKind::Unknown => Verdict::Unknown,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    // Only the tests build interfaces by hand; the module itself reads them.
    use network_interface::{Addr, V6IfAddr};

    fn hop(interface: &str, kind: InterfaceKind) -> Hop {
        Hop {
            interface: interface.to_owned(),
            kind,
        }
    }

    // --- The probe ------------------------------------------------------

    #[test]
    fn an_ipv4_route_resolves_to_a_real_source_address() {
        // Any machine running this test has a v4 route or no network at all;
        // CI has one. What is being pinned is that the trick works: a bound,
        // connected UDP socket reports the address a peer would see.
        let source = source_address(PROBE_V4);
        if let Some(ip) = source {
            assert!(!ip.is_unspecified(), "0.0.0.0 is not an answer");
            assert!(ip.is_ipv4(), "a v4 probe must yield a v4 source");
        }
    }

    #[test]
    fn the_probe_names_an_interface_that_exists() {
        // The end-to-end shape: whatever the path is, an interface it names
        // has to be one the platform also enumerates. A name that matches
        // nothing means `hop_for` matched on the wrong field.
        let path = probe();
        let interfaces = NetworkInterface::show().unwrap_or_default();

        for hop in [path.v4.as_ref(), path.v6.as_ref()].into_iter().flatten() {
            assert!(
                interfaces.iter().any(|i| i.name == hop.interface),
                "probe named {:?}, which is not in the interface list",
                hop.interface
            );
        }
    }

    /// Prints what [`classify`] is handed on this machine, and what it makes
    /// of it.
    ///
    /// Not an assertion — a window. The classifier has to work against strings
    /// three operating systems choose for themselves, and every other way of
    /// reading those strings gives a *different* one: on Windows this crate
    /// reports the adapter's `FriendlyName`, while `ipconfig /all` prints its
    /// Description and `Get-NetAdapter` prints two further names again.
    /// Classifying against the wrong one of those four fails only on real
    /// hardware, months later.
    ///
    /// So this reads them the way production will. Run it on each platform,
    /// once with the VPN connected and once without — the disconnected run is
    /// the one that proves the classifier does not hand out false positives.
    ///
    /// ```text
    /// cargo test --lib egress::tests::show_this_machines_interfaces -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints this machine's interfaces; run by hand when teaching the classifier a platform"]
    fn show_this_machines_interfaces() {
        let path = probe();
        let routed: Vec<&str> = [path.v4.as_ref(), path.v6.as_ref()]
            .into_iter()
            .flatten()
            .map(|hop| hop.interface.as_str())
            .collect();

        println!("\nplatform: {}", std::env::consts::OS);
        println!(
            "IPv4 leaves by: {}",
            path.v4.as_ref().map_or("(no route)", |h| &h.interface)
        );
        println!(
            "IPv6 leaves by: {}",
            path.v6.as_ref().map_or("(no route)", |h| &h.interface)
        );
        println!("verdict (no pin): {:?}\n", path.verdict(None));

        println!(
            "{:<4} {:<34} {:<19} {:<9} {:?}",
            "", "name (as the crate reports it)", "mac_addr", "internal", "classify()"
        );
        for interface in NetworkInterface::show().unwrap_or_default() {
            println!(
                "{:<4} {:<34} {:<19} {:<9} {:?}",
                if routed.contains(&interface.name.as_str()) {
                    "-->"
                } else {
                    ""
                },
                interface.name,
                interface.mac_addr.as_deref().unwrap_or("(none)"),
                interface.internal,
                classify(
                    &interface.name,
                    interface.mac_addr.as_deref(),
                    interface.internal
                )
            );
        }
        println!("\n--> marks an interface currently carrying traffic.\n");
        println!("pin picker would offer, in order:");
        for hop in candidates() {
            println!("  {:<20} {:?}", hop.interface, hop.kind);
        }
    }

    // --- Classification -------------------------------------------------
    //
    // These are the specification for `classify`. They fail until it is
    // written, and that is deliberate.

    #[test]
    fn classification_recognises_a_macos_tunnel() {
        // The shape every `utun` has: point-to-point, so no MAC at all.
        assert_eq!(classify("utun6", None, false), InterfaceKind::Tunnel);
    }

    #[test]
    fn classification_recognises_linux_tunnels() {
        // `torguard-wg` is measured on Fedora with TorGuard connected; it
        // reported no MAC while `eth0` reported 00:15:5d:7f:f7:07. The rest
        // are the conventional names.
        //
        // Note what `torguard-wg` costs: it is the same vendor as Windows'
        // `wg-torguard` with the components reversed, so no prefix rule
        // catches both and no rule at all catches this one by name. The
        // absent MAC is what identifies it.
        for name in ["torguard-wg", "wg0", "tun0", "ppp0"] {
            assert_eq!(
                classify(name, None, false),
                InterfaceKind::Tunnel,
                "{name} should read as a tunnel"
            );
        }
    }

    #[test]
    fn classification_recognises_a_windows_wireguard_adapter() {
        // Measured, not guessed. TorGuard's WireGuard adapter on Windows 11
        // ARM reports the friendly name `wg-torguard`; its
        // InterfaceDescription is "WireGuard Tunnel", which `classify` never
        // sees.
        //
        // Note what that name actually is: WireGuard for Windows names the
        // adapter after the tunnel *config file*, so this is TorGuard's choice
        // of config name and not a vendor string. It holds for TorGuard users
        // because TorGuard generates the config, and it would not hold for a
        // hand-rolled one.
        assert_eq!(classify("wg-torguard", None, false), InterfaceKind::Tunnel);
    }

    #[test]
    fn classification_refuses_to_guess_at_a_name_that_says_nothing() {
        // The case that rules name-matching out as a general Windows strategy.
        // TorGuard's OpenVPN adapter is a TAP-Windows Adapter V9 whose
        // friendly name is "Local Area Connection" -- and it sits in the
        // adapter list directly beside "Local Area Connection* 6", a WAN
        // Miniport that is not a tunnel at all. No substring separates them.
        //
        // So both fail closed. The first is a false negative: with TorGuard in
        // OpenVPN mode the guard holds transfer that is in fact tunnelled,
        // which is the cheap direction to be wrong in and what the interface
        // pin exists to fix. Matching "Local Area Connection" would trade that
        // for a false positive on a WAN Miniport, which is the expensive one.
        // Both carry a MAC as well, so neither signal reaches them: the TAP
        // adapter measured 00-FF-FD-61-9F-3D.
        for (name, mac) in [
            ("Local Area Connection", Some("00FFFD619F3D")),
            ("Local Area Connection* 6", Some("00FFFD619F3E")),
        ] {
            assert_ne!(
                classify(name, mac, false),
                InterfaceKind::Tunnel,
                "{name} must not read as a tunnel; nothing in it says tunnel"
            );
        }
    }

    #[test]
    fn classification_does_not_mistake_ethernet_or_wifi_for_a_tunnel() {
        // The failure that matters most: telling someone they are covered.
        // Measured pairs. "Ethernet" is the Parallels VirtIO adapter carrying
        // traffic on the Windows VM; its MAC is real and so is en7's, though
        // macOS reports a placeholder rather than that value.
        for (name, mac) in [
            ("en7", Some("02:00:00:00:00:00")),
            ("Ethernet", Some("001C42A6858B")),
            ("eth0", Some("00:15:5d:7f:f7:07")),
            ("Wi-Fi", Some("a4:83:e7:00:11:22")),
            ("enp3s0", Some("00:15:5d:01:02:03")),
            ("wlan0", Some("00:15:5d:01:02:04")),
        ] {
            assert_eq!(
                classify(name, mac, false),
                InterfaceKind::Ordinary,
                "{name} must not read as a tunnel"
            );
        }
    }

    #[test]
    fn classification_does_not_mistake_loopback_for_a_tunnel() {
        // Measured: loopback reports an all-zero MAC rather than no MAC, on
        // both macOS (`lo0`) and Linux (`lo`). So a rule that normalised
        // "00:00:00:00:00:00" to absent would land loopback in the tunnel
        // bucket, and `internal` is what keeps it out. It is set on exactly
        // these two and nothing else on any platform measured.
        assert_ne!(
            classify("lo0", Some("00:00:00:00:00:00"), true),
            InterfaceKind::Tunnel
        );
        assert_ne!(
            classify("lo", Some("00:00:00:00:00:00"), true),
            InterfaceKind::Tunnel
        );
    }

    #[test]
    fn an_absent_mac_reads_as_a_tunnel_even_under_an_unfamiliar_name() {
        // The case that makes the MAC worth taking: WireGuard for Windows
        // names the adapter after the config file, so a user whose config is
        // "TorGuard-US-East.conf" gets an adapter no name rule will match.
        // It still reports no physical address.
        assert_eq!(
            classify("TorGuard-US-East", None, false),
            InterfaceKind::Tunnel
        );
    }

    #[test]
    fn a_present_mac_is_never_evidence_against_a_tunnel() {
        // macOS reports a placeholder MAC for every interface including every
        // utun, so a rule that read a present MAC as "ordinary" would classify
        // the entire platform wrong.
        assert_eq!(
            classify("utun6", Some("00:00:00:00:00:00"), false),
            InterfaceKind::Tunnel
        );
        assert_eq!(
            classify("wg0", Some("02:00:00:00:00:00"), false),
            InterfaceKind::Tunnel
        );
    }

    #[test]
    fn a_tunnel_that_carries_no_privacy_is_not_reported_as_one() {
        // 6to4, IPIP and GRE are tunnels in the networking sense and plaintext
        // to everyone on the path. Reporting one as a tunnel would tell a user
        // they are protected at a moment when they are not.
        //
        // `sit0` and `gre0` are given no MAC here because that is what Linux
        // reports for them -- without this rule the MAC would carry them
        // straight to Tunnel.
        for (name, mac) in [
            ("gif0", Some("00:00:00:00:00:00")),
            ("stf0", Some("00:00:00:00:00:00")),
            ("sit0", None),
            ("gre0", None),
            ("tunl0", None),
        ] {
            assert_eq!(
                classify(name, mac, false),
                InterfaceKind::Unknown,
                "{name} carries no privacy and must not read as a tunnel"
            );
        }

        // The neighbouring name that *is* a real tunnel still is one.
        assert_eq!(classify("tun0", None, false), InterfaceKind::Tunnel);
    }

    #[test]
    fn classification_is_not_confused_by_case() {
        // macOS and Linux are lower case; Windows friendly names are whatever
        // the vendor typed.
        // These carry a MAC, so they can only pass through the name rule --
        // otherwise they would succeed for the wrong reason.
        assert_eq!(
            classify("UTUN6", Some("00:00:00:00:00:00"), false),
            InterfaceKind::Tunnel
        );
        assert_eq!(
            classify("WG0", Some("02:00:00:00:00:00"), false),
            InterfaceKind::Tunnel
        );
        assert_eq!(
            classify("Ethernet", Some("001C42A6858B"), false),
            InterfaceKind::Ordinary
        );
    }

    // --- The picker list --------------------------------------------------

    #[test]
    fn the_candidate_list_puts_tunnels_first_and_leaves_loopback_out() {
        let hops = candidates();

        assert!(
            !hops
                .iter()
                .any(|hop| hop.interface == "lo0" || hop.interface == "lo"),
            "pinning loopback would hold transfer permanently: {hops:?}"
        );

        let ranks: Vec<u8> = hops
            .iter()
            .map(|hop| match hop.kind {
                InterfaceKind::Tunnel => 0,
                InterfaceKind::Ordinary => 1,
                InterfaceKind::Unknown => 2,
            })
            .collect();
        assert!(
            ranks.windows(2).all(|pair| pair[0] <= pair[1]),
            "someone opening this list is looking for their VPN: {hops:?}"
        );
    }

    #[test]
    fn the_candidate_list_names_each_interface_once() {
        // The platform lists an interface once per address family; a picker
        // showing en7 twice is a picker nobody trusts.
        let hops = candidates();
        let mut names: Vec<&str> = hops.iter().map(|hop| hop.interface.as_str()).collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len(), "duplicate interfaces in {hops:?}");
    }

    // --- The hysteresis gate ---------------------------------------------

    /// A fixed origin plus offsets, so the hysteresis is tested without
    /// sleeping and without a real clock.
    fn at(base: Instant, seconds: u64) -> Instant {
        base + Duration::from_secs(seconds)
    }

    #[test]
    fn a_gate_starts_held_rather_than_open() {
        // A gate that began open would permit transfer for the first tick of
        // its life, which is the launch window this feature exists to close.
        assert_eq!(TransferGate::default().gate(), Gate::Held);
    }

    #[test]
    fn a_failing_verdict_holds_on_the_very_first_tick() {
        // No hysteresis on the way in. Protection is never delayed.
        let base = Instant::now();
        let mut gate = TransferGate::default();
        gate.release_now();
        assert_eq!(gate.gate(), Gate::Open);

        assert_eq!(gate.observe(false, base), Gate::Held);
    }

    #[test]
    fn recovery_waits_for_the_settle_window() {
        let base = Instant::now();
        let mut gate = TransferGate::new(Duration::from_secs(10));

        assert_eq!(gate.observe(false, at(base, 0)), Gate::Held);
        // Permission starts here, but the window has not run.
        assert_eq!(gate.observe(true, at(base, 1)), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 5)), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 10)), Gate::Held);
        // t=1 plus ten seconds.
        assert_eq!(gate.observe(true, at(base, 11)), Gate::Open);
    }

    #[test]
    fn a_flap_during_the_settle_window_resets_it_rather_than_shortening_it() {
        // The case the window exists for: a VPN reconnecting onto a new
        // interface, or a laptop waking. Releasing into the middle of one
        // costs a full re-announce to every tracker, per torrent.
        let base = Instant::now();
        let mut gate = TransferGate::new(Duration::from_secs(10));

        assert_eq!(gate.observe(true, at(base, 0)), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 8)), Gate::Held);
        // Nine seconds in, it drops again.
        assert_eq!(gate.observe(false, at(base, 9)), Gate::Held);
        // Back, but the clock restarts from here rather than resuming at 9.
        assert_eq!(gate.observe(true, at(base, 10)), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 19)), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 20)), Gate::Open);
    }

    #[test]
    fn an_open_gate_stays_open_without_re_settling() {
        // Once open, a continuing permission must not restart the window --
        // that would hold transfer for ten seconds out of every ten.
        let base = Instant::now();
        let mut gate = TransferGate::new(Duration::from_secs(10));
        gate.release_now();

        for second in 0..30 {
            assert_eq!(
                gate.observe(true, at(base, second)),
                Gate::Open,
                "second {second} should stay open"
            );
        }
    }

    #[test]
    fn the_remaining_wait_is_reportable_so_the_ui_can_say_why() {
        // "Held" is not a status. "A tunnel is back; transfer resumes in 6 s"
        // is one.
        let base = Instant::now();
        let mut gate = TransferGate::new(Duration::from_secs(10));

        gate.observe(false, at(base, 0));
        assert_eq!(gate.settling_for(at(base, 0)), None, "not settling yet");

        gate.observe(true, at(base, 4));
        assert_eq!(
            gate.settling_for(at(base, 4)),
            Some(Duration::from_secs(10))
        );
        assert_eq!(gate.settling_for(at(base, 8)), Some(Duration::from_secs(6)));

        gate.observe(true, at(base, 15));
        assert_eq!(gate.settling_for(at(base, 15)), None, "open, not settling");
    }

    #[test]
    fn a_user_edit_releases_without_waiting_out_the_window() {
        // Someone who has just retyped an interface name is watching the
        // window. Ten seconds of nothing is how a settle period becomes a bug
        // report.
        let base = Instant::now();
        let mut gate = TransferGate::default();
        gate.observe(false, at(base, 0));

        gate.release_now();
        assert_eq!(gate.gate(), Gate::Open);
        assert_eq!(gate.observe(true, at(base, 1)), Gate::Open);
    }

    #[test]
    fn switching_into_hold_while_already_failing_trips_at_once() {
        // Carrying over a settle window that started under the old policy
        // would let transfer through on the strength of a verdict nobody
        // judged against the new one.
        let base = Instant::now();
        let mut gate = TransferGate::default();
        gate.release_now();

        gate.hold_now();
        assert_eq!(gate.gate(), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 1)), Gate::Held);
        assert_eq!(gate.observe(true, at(base, 20)), Gate::Open);
    }

    // --- The watcher's cache --------------------------------------------

    /// An interface owning one IPv4 address, built the way the crate does.
    fn iface(name: &str, ip: [u8; 4], index: u32) -> NetworkInterface {
        NetworkInterface::new_afinet(name, std::net::Ipv4Addr::from(ip), None, None, index, false)
    }

    /// Counts how many times the watcher asked for the interface list.
    struct Counter(std::cell::Cell<usize>);

    impl Counter {
        fn new() -> Self {
            Self(std::cell::Cell::new(0))
        }
        fn count(&self) -> usize {
            self.0.get()
        }
        /// Counts on **call**, not on construction.
        ///
        /// Getting this backwards is easy and silently defeats the test: the
        /// watcher builds a closure every round and calls it only on a miss,
        /// so a counter that ticked at build time would count rounds and
        /// assert nothing about caching at all.
        fn enumerate(
            &self,
            interfaces: Vec<NetworkInterface>,
        ) -> impl FnOnce() -> Vec<NetworkInterface> + '_ {
            move || {
                self.0.set(self.0.get() + 1);
                interfaces
            }
        }
    }

    #[test]
    fn an_unchanged_source_address_does_not_re_enumerate() {
        // The entire reason the watcher exists. An enumeration is 3.2 ms
        // against a 27.7 µs route lookup, and it is asked for once here rather
        // than once a second.
        let mut watcher = EgressWatcher::default();
        let source = Some(IpAddr::from([192, 168, 1, 10]));
        let counter = Counter::new();
        let list = || vec![iface("en7", [192, 168, 1, 10], 1)];

        let first = watcher.resolve(source, None, counter.enumerate(list()));
        assert_eq!(counter.count(), 1, "the first read has nothing cached");
        assert_eq!(first.v4.as_ref().map(|h| h.interface.as_str()), Some("en7"));

        for _ in 0..10 {
            let again = watcher.resolve(source, None, counter.enumerate(list()));
            assert_eq!(again, first, "the cached answer must not drift");
        }
        assert_eq!(
            counter.count(),
            1,
            "eleven reads of an unchanged address must cost one enumeration"
        );
    }

    #[test]
    fn a_changed_source_address_re_enumerates_on_that_very_tick() {
        // This is what makes the cache safe for a kill switch: it is keyed on
        // the address rather than on a clock, so a VPN dropping is noticed on
        // the tick it happens rather than after a staleness window.
        let mut watcher = EgressWatcher::default();
        let counter = Counter::new();

        let tunnelled = Some(IpAddr::from([10, 13, 57, 109]));
        let direct = Some(IpAddr::from([192, 168, 1, 10]));
        let list = || {
            vec![
                iface("utun12", [10, 13, 57, 109], 20),
                iface("en7", [192, 168, 1, 10], 1),
            ]
        };

        let before = watcher.resolve(tunnelled, None, counter.enumerate(list()));
        assert_eq!(
            before.v4.as_ref().map(|h| h.interface.as_str()),
            Some("utun12")
        );

        assert_eq!(counter.count(), 1);

        let after = watcher.resolve(direct, None, counter.enumerate(list()));
        assert_eq!(after.v4.as_ref().map(|h| h.interface.as_str()), Some("en7"));
        assert_eq!(
            after.verdict(None),
            Verdict::Direct {
                interface: "en7".into()
            }
        );
        assert_eq!(
            counter.count(),
            2,
            "an address that moved must be resolved again, not served from cache"
        );
    }

    #[test]
    fn losing_a_route_clears_that_family_rather_than_reporting_a_stale_one() {
        // Measured behaviour: TorGuard removes the IPv6 route entirely while
        // connected. Keeping the last interface would have the UI naming
        // something nothing is leaving by.
        let mut watcher = EgressWatcher::default();
        let counter = Counter::new();
        let v6 = Some(IpAddr::from([0x2603, 0, 0, 0, 0, 0, 0, 1]));

        let list = || {
            let mut en7 = NetworkInterface::new_afinet6(
                "en7",
                std::net::Ipv6Addr::new(0x2603, 0, 0, 0, 0, 0, 0, 1),
                None,
                None,
                1,
                false,
            );
            en7.mac_addr = Some("02:00:00:00:00:00".into());
            vec![en7]
        };

        let before = watcher.resolve(None, v6, counter.enumerate(list()));
        assert!(before.v6.is_some(), "the v6 hop should resolve");

        let after = watcher.resolve(None, None, counter.enumerate(list()));
        assert_eq!(
            after,
            EgressPath::default(),
            "a lost route leaves nothing behind"
        );
    }

    #[test]
    fn a_family_with_no_route_never_costs_an_enumeration() {
        // A v4-only network must not pay 3.2 ms every tick for an IPv6 lookup
        // that has nothing to resolve.
        let mut watcher = EgressWatcher::default();
        let counter = Counter::new();
        let source = Some(IpAddr::from([192, 168, 1, 10]));
        let list = || vec![iface("en7", [192, 168, 1, 10], 1)];

        watcher.resolve(source, None, counter.enumerate(list()));
        assert_eq!(counter.count(), 1, "the first read resolves v4");

        for _ in 0..5 {
            watcher.resolve(source, None, counter.enumerate(list()));
        }
        assert_eq!(
            counter.count(),
            1,
            "an absent IPv6 route must never force an enumeration of its own"
        );
    }

    #[test]
    fn both_families_missing_together_enumerate_only_once() {
        // One walk of the interface list answers both, and the walk is the
        // entire cost.
        let mut watcher = EgressWatcher::default();
        let v4 = Some(IpAddr::from([10, 13, 57, 109]));
        let v6 = Some(IpAddr::from([0x2603, 0, 0, 0, 0, 0, 0, 1]));

        let calls = std::cell::Cell::new(0);
        let path = watcher.resolve(v4, v6, || {
            calls.set(calls.get() + 1);
            let mut utun = NetworkInterface::new_afinet(
                "utun12",
                std::net::Ipv4Addr::new(10, 13, 57, 109),
                None,
                None,
                20,
                false,
            );
            utun.addr.push(Addr::V6(V6IfAddr {
                ip: std::net::Ipv6Addr::new(0x2603, 0, 0, 0, 0, 0, 0, 1),
                broadcast: None,
                netmask: None,
            }));
            vec![utun]
        });

        assert_eq!(calls.get(), 1, "two misses, one walk");
        assert_eq!(
            path.v4.as_ref().map(|h| h.interface.as_str()),
            Some("utun12")
        );
        assert_eq!(
            path.v6.as_ref().map(|h| h.interface.as_str()),
            Some("utun12")
        );
    }

    // --- The verdict ----------------------------------------------------

    #[test]
    fn a_tunnel_on_both_families_is_tunnelled_and_says_nothing_leaks() {
        let path = EgressPath {
            v4: Some(hop("utun6", InterfaceKind::Tunnel)),
            v6: Some(hop("utun6", InterfaceKind::Tunnel)),
        };

        assert_eq!(
            path.verdict(None),
            Verdict::Tunnelled {
                interface: "utun6".into(),
                other_family_outside: false,
            }
        );
    }

    #[test]
    fn ipv6_outside_the_tunnel_is_reported_without_failing_the_verdict() {
        // The decision on this feature: v4 decides, v6 is reported alongside.
        // The development machine can produce exactly this — en7 carries a
        // global v6 address and its own default route.
        let path = EgressPath {
            v4: Some(hop("utun6", InterfaceKind::Tunnel)),
            v6: Some(hop("en7", InterfaceKind::Ordinary)),
        };

        let verdict = path.verdict(None);
        assert_eq!(
            verdict,
            Verdict::Tunnelled {
                interface: "utun6".into(),
                other_family_outside: true,
            }
        );
        assert!(
            verdict.allows_transfer(),
            "a v6 leak is reported, not enforced against"
        );
    }

    #[test]
    fn the_wire_shape_is_camel_case_on_both_the_tag_and_the_fields() {
        // The mirror in `src/lib/ipc/types.ts` reads these names. A field that
        // went out as `other_family_outside` would parse as `undefined` on the
        // other side, and the v6 leak warning would never render.
        let json = serde_json::to_value(Verdict::Tunnelled {
            interface: "utun6".into(),
            other_family_outside: true,
        })
        .expect("serialises");

        assert_eq!(json["verdict"], "tunnelled");
        assert_eq!(json["otherFamilyOutside"], true);
        assert_eq!(json["interface"], "utun6");

        let json = serde_json::to_value(Verdict::WrongTunnel {
            interface: "utun7".into(),
            expected: "utun6".into(),
        })
        .expect("serialises");
        assert_eq!(json["verdict"], "wrongTunnel");
    }

    #[test]
    fn an_ordinary_adapter_is_direct() {
        let path = EgressPath {
            v4: Some(hop("en7", InterfaceKind::Ordinary)),
            v6: Some(hop("en7", InterfaceKind::Ordinary)),
        };

        assert_eq!(
            path.verdict(None),
            Verdict::Direct {
                interface: "en7".into()
            }
        );
        assert!(!path.verdict(None).allows_transfer());
    }

    #[test]
    fn a_tunnel_that_is_not_the_pinned_one_is_its_own_verdict() {
        // Distinct from Direct because the remedy is different: re-pin, rather
        // than connect something.
        let path = EgressPath {
            v4: Some(hop("utun7", InterfaceKind::Tunnel)),
            v6: None,
        };

        assert_eq!(
            path.verdict(Some("utun6")),
            Verdict::WrongTunnel {
                interface: "utun7".into(),
                expected: "utun6".into(),
            }
        );
    }

    #[test]
    fn a_pin_is_matched_the_way_a_person_would_type_it() {
        // The pin is free text in a settings field; the interface name comes
        // from the OS. Byte-exact comparison between those two strands the
        // library on a capitalisation the user cannot see.
        let path = EgressPath {
            v4: Some(hop("Ethernet", InterfaceKind::Ordinary)),
            v6: None,
        };

        for typed in [
            "Ethernet",
            "ethernet",
            "ETHERNET",
            "  Ethernet  ",
            "ethernet ",
        ] {
            assert!(
                path.verdict(Some(typed)).allows_transfer(),
                "{typed:?} should match the interface named Ethernet"
            );
        }

        // Still a different interface, not merely differently spelled.
        assert!(!path.verdict(Some("en7")).allows_transfer());
    }

    #[test]
    fn pinning_the_interface_that_is_actually_carrying_traffic_passes() {
        let path = EgressPath {
            v4: Some(hop("utun6", InterfaceKind::Tunnel)),
            v6: None,
        };
        assert!(path.verdict(Some("utun6")).allows_transfer());
    }

    #[test]
    fn a_pin_rescues_an_interface_the_classifier_cannot_identify() {
        // TorGuard in OpenVPN mode on Windows: "Local Area Connection", with a
        // MAC and an 802.3 media type, unreachable by any signal this crate
        // exposes. Without this the pin would rescue nothing.
        let path = EgressPath {
            v4: Some(hop("Local Area Connection", InterfaceKind::Ordinary)),
            v6: None,
        };

        let verdict = path.verdict(Some("Local Area Connection"));
        assert_eq!(
            verdict,
            Verdict::Pinned {
                interface: "Local Area Connection".into(),
                other_family_outside: false,
            }
        );
        assert!(verdict.allows_transfer());
    }

    #[test]
    fn a_pinned_interface_that_is_a_tunnel_still_reports_as_tunnelled() {
        // The stronger claim is available here, so it is the one made.
        let path = EgressPath {
            v4: Some(hop("wg-torguard", InterfaceKind::Tunnel)),
            v6: None,
        };
        assert!(matches!(
            path.verdict(Some("wg-torguard")),
            Verdict::Tunnelled { .. }
        ));
    }

    #[test]
    fn traffic_dying_at_the_wireguard_kill_switch_holds_rather_than_passes() {
        // A real observed state, not a hypothetical. WireGuard for Windows
        // installs 0.0.0.0/1 and 128.0.0.0/1 blackholes pointing at loopback,
        // beaten by four /2 routes on the tunnel while it is up. In the window
        // where the adapter exists but its routes do not, the route to the
        // internet resolves to "Loopback Pseudo-Interface 1" -- which is
        // traffic going nowhere, and must not read as either tunnelled or
        // direct.
        let path = EgressPath {
            v4: Some(hop("Loopback Pseudo-Interface 1", InterfaceKind::Unknown)),
            v6: None,
        };
        assert_eq!(path.verdict(None), Verdict::Unknown);
        assert!(!path.verdict(None).allows_transfer());
    }

    #[test]
    fn no_route_at_all_is_unknown_and_holds_traffic() {
        let path = EgressPath::default();
        assert_eq!(path.verdict(None), Verdict::Unknown);
        assert!(
            !path.verdict(None).allows_transfer(),
            "a guard that fails open is decoration"
        );
    }

    #[test]
    fn an_unclassifiable_interface_is_unknown_rather_than_direct() {
        // Rule 9: not a guess between the two answers.
        let path = EgressPath {
            v4: Some(hop("mystery0", InterfaceKind::Unknown)),
            v6: None,
        };
        assert_eq!(path.verdict(None), Verdict::Unknown);
    }

    #[test]
    fn a_v6_only_network_is_judged_on_v6_rather_than_held_forever() {
        // No v4 route to decide on. Refusing to judge would hold every
        // transfer on a network that is working correctly.
        let path = EgressPath {
            v4: None,
            v6: Some(hop("utun6", InterfaceKind::Tunnel)),
        };

        assert_eq!(
            path.verdict(None),
            Verdict::Tunnelled {
                interface: "utun6".into(),
                other_family_outside: false,
            }
        );
    }

    #[test]
    fn a_v6_only_network_on_an_ordinary_adapter_still_fails() {
        let path = EgressPath {
            v4: None,
            v6: Some(hop("en7", InterfaceKind::Ordinary)),
        };
        assert!(!path.verdict(None).allows_transfer());
    }
}
