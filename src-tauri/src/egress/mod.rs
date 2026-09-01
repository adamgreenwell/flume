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
//! Like [`crate::engine`], this module imports no Tauri types and runs under a
//! plain `cargo test`.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};

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
    // TODO(adam): implement. See `classification_*` tests below for the cases
    // this has to satisfy — they are the specification, and they fail until
    // this is written.
    let _ = (name, mac_addr, internal);
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

/// Reads the current egress path for both address families.
///
/// Two route lookups and one interface enumeration. Nothing is sent and
/// nothing is cached — the caller decides how often this is worth asking, and
/// [`crate::telemetry`] does not ask every tick.
#[must_use]
pub fn probe() -> EgressPath {
    let interfaces = NetworkInterface::show().unwrap_or_default();

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
            if expected != deciding.interface {
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
    fn classification_is_not_confused_by_case() {
        // macOS and Linux are lower case; Windows friendly names are whatever
        // the vendor typed.
        assert_eq!(classify("UTUN6", None, false), InterfaceKind::Tunnel);
        assert_eq!(
            classify("wireguard tunnel", None, false),
            InterfaceKind::Tunnel
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
