//! Embedded EasyTier engine — supports **multiple concurrent networks**.
//!
//! A dedicated OS thread runs a multi-threaded Tokio runtime that owns a map of
//! live [`NetworkInstance`]s keyed by profile id. Each tick it refreshes every
//! instance and republishes an immutable [`Snapshot`]. The single-threaded WinUI
//! render loop only ever sends commands and reads snapshots.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use easytier::common::config::ConfigFileControl;
use easytier::launcher::{NetworkConfig, NetworkInstance};
use tokio::sync::mpsc;

/// Connection lifecycle as surfaced to the UI.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Disconnected => "Disconnected",
            Status::Connecting => "Connecting…",
            Status::Connected => "Connected",
            Status::Error => "Error",
        }
    }
}

/// One row in a peers table — already formatted for display.
#[derive(Clone)]
pub struct PeerRow {
    pub hostname: String,
    pub ipv4: String,
    pub cost: String,
    /// `None` for relayed peers with no direct connection (rendered as "—").
    pub latency_ms: Option<f64>,
    pub tunnel: String,
    pub loss: f32,
    pub rx: u64,
    pub tx: u64,
    pub nat: String,
    pub version: String,
}

/// State of a single network instance.
#[derive(Clone, Default)]
pub struct NetSnapshot {
    pub status: Status,
    pub virtual_ip: String,
    pub hostname: String,
    pub peer_id: u32,
    pub version: String,
    pub dev_name: String,
    pub nat_type: String,
    pub listeners: Vec<String>,
    pub public_ipv4: String,
    pub public_ipv6: String,
    pub vpn_portal_cfg: Option<String>,
    pub peers: Vec<PeerRow>,
    pub events: Vec<String>,
    pub error: Option<String>,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Immutable view of all engine state, keyed by profile id.
#[derive(Clone, Default)]
pub struct Snapshot {
    pub nets: HashMap<String, NetSnapshot>,
}

impl Snapshot {
    pub fn net(&self, id: &str) -> Option<&NetSnapshot> {
        self.nets.get(id)
    }

    /// A network is "live" if it has an instance that is connected or connecting.
    pub fn is_live(&self, id: &str) -> bool {
        self.nets
            .get(id)
            .is_some_and(|n| matches!(n.status, Status::Connected | Status::Connecting))
    }

    pub fn connected_count(&self) -> usize {
        self.nets
            .values()
            .filter(|n| matches!(n.status, Status::Connected))
            .count()
    }

    pub fn total_peers(&self) -> usize {
        self.nets.values().map(|n| n.peers.len()).sum()
    }

    pub fn total_rx(&self) -> u64 {
        self.nets.values().map(|n| n.rx_bytes).sum()
    }

    pub fn total_tx(&self) -> u64 {
        self.nets.values().map(|n| n.tx_bytes).sum()
    }

    /// One-line connection summary for the system-tray tooltip.
    pub fn status_summary(&self) -> String {
        use crate::i18n::{t, tn};
        let connected = self.connected_count();
        if connected > 0 {
            format!(
                "{} · {}",
                tn("{n} connected", connected),
                tn("{n} peers", self.total_peers())
            )
        } else if self
            .nets
            .values()
            .any(|n| matches!(n.status, Status::Connecting))
        {
            t("Connecting…")
        } else if self
            .nets
            .values()
            .any(|n| matches!(n.status, Status::Error))
        {
            t("Error")
        } else {
            t("Not connected")
        }
    }
}

enum Cmd {
    Start { id: String, cfg: Box<NetworkConfig> },
    Stop { id: String },
}

/// Handle to the background EasyTier worker. Cloneable and `Send`/`Sync`.
#[derive(Clone)]
pub struct Engine {
    tx: mpsc::UnboundedSender<Cmd>,
    state: Arc<Mutex<Snapshot>>,
}

impl Engine {
    pub fn new() -> Self {
        let state = Arc::new(Mutex::new(Snapshot::default()));
        let (tx, rx) = mpsc::unbounded_channel::<Cmd>();
        let worker_state = state.clone();
        thread::Builder::new()
            .name("polaris-easytier".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(2)
                    .build()
                    .expect("failed to build EasyTier runtime");
                rt.block_on(worker(rx, worker_state));
            })
            .expect("failed to spawn EasyTier worker thread");
        Engine { tx, state }
    }

    /// Bring up (or restart) the network with the given profile id.
    pub fn start(&self, id: impl Into<String>, cfg: NetworkConfig) {
        let _ = self.tx.send(Cmd::Start {
            id: id.into(),
            cfg: Box::new(cfg),
        });
    }

    /// Tear down the network with the given profile id.
    pub fn stop(&self, id: impl Into<String>) {
        let _ = self.tx.send(Cmd::Stop { id: id.into() });
    }

    /// Cheap clone of the latest published state.
    pub fn snapshot(&self) -> Snapshot {
        self.state.lock().unwrap().clone()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

async fn worker(mut rx: mpsc::UnboundedReceiver<Cmd>, state: Arc<Mutex<Snapshot>>) {
    let mut instances: HashMap<String, NetworkInstance> = HashMap::new();
    // Networks whose config failed to apply — kept so the UI can show the error.
    let mut errors: HashMap<String, String> = HashMap::new();

    let mut ticker = tokio::time::interval(Duration::from_millis(1000));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            cmd = rx.recv() => match cmd {
                None => break, // app exiting
                Some(Cmd::Stop { id }) => {
                    instances.remove(&id); // Drop stops the launcher thread.
                    errors.remove(&id);
                    publish(&instances, &errors, &state).await;
                }
                Some(Cmd::Start { id, mut cfg }) => {
                    errors.remove(&id);
                    instances.remove(&id);
                    // Creating a TUN adapter needs admin. Without it, fall back
                    // to proxy-only (SOCKS5 / port-forward) so the connection
                    // still comes up instead of failing.
                    if !crate::elevate::is_elevated() {
                        cfg.no_tun = Some(true);
                    }
                    match cfg.gen_config() {
                        Ok(toml_cfg) => {
                            let mut ni = NetworkInstance::new(toml_cfg, ConfigFileControl::STATIC_CONFIG);
                            match ni.start() {
                                Ok(_) => { instances.insert(id, ni); }
                                Err(e) => { errors.insert(id, format!("{e:?}")); }
                            }
                        }
                        Err(e) => { errors.insert(id, format!("Invalid configuration: {e}")); }
                    }
                    publish(&instances, &errors, &state).await;
                }
            },
            _ = ticker.tick() => {
                if !instances.is_empty() {
                    publish(&instances, &errors, &state).await;
                }
            }
        }
    }
}

/// Rebuild and store a fresh snapshot from the live instances + recorded errors.
async fn publish(
    instances: &HashMap<String, NetworkInstance>,
    errors: &HashMap<String, String>,
    state: &Arc<Mutex<Snapshot>>,
) {
    let mut snap = Snapshot::default();

    for (id, ni) in instances {
        snap.nets.insert(id.clone(), net_snapshot(ni).await);
    }
    for (id, msg) in errors {
        snap.nets.insert(
            id.clone(),
            NetSnapshot {
                status: Status::Error,
                error: Some(msg.clone()),
                ..NetSnapshot::default()
            },
        );
    }

    *state.lock().unwrap() = snap;
}

async fn net_snapshot(ni: &NetworkInstance) -> NetSnapshot {
    let info = match ni.get_running_info().await {
        Ok(info) => info,
        Err(_) => {
            // RPC not ready yet, or shutting down.
            return NetSnapshot {
                status: ni
                    .get_latest_error_msg()
                    .as_ref()
                    .map_or(Status::Connecting, |_| Status::Error),
                error: ni.get_latest_error_msg(),
                ..NetSnapshot::default()
            };
        }
    };

    let mut net = NetSnapshot {
        status: if info.running {
            Status::Connected
        } else {
            Status::Connecting
        },
        error: info.error_msg.clone(),
        dev_name: info.dev_name.clone(),
        ..NetSnapshot::default()
    };

    if let Some(node) = &info.my_node_info {
        net.virtual_ip = node
            .virtual_ipv4
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_default();
        net.hostname = node.hostname.clone();
        net.peer_id = node.peer_id;
        net.version = node.version.clone();
        net.vpn_portal_cfg = node.vpn_portal_cfg.clone();
        net.listeners = node.listeners.iter().map(|l| l.url.clone()).collect();
        if let Some(stun) = &node.stun_info {
            net.nat_type = format!("{:?}", stun.udp_nat_type());
        }
        if let Some(ips) = &node.ips {
            if let Some(v4) = &ips.public_ipv4 {
                net.public_ipv4 = v4.to_string();
            }
            if let Some(v6) = &ips.public_ipv6 {
                net.public_ipv6 = v6.to_string();
            }
        }
    }

    net.events = info
        .events
        .iter()
        .rev()
        .take(200)
        .map(|e| pretty_event(e))
        .collect();

    for pair in &info.peer_route_pairs {
        let Some(route) = &pair.route else { continue };

        let mut rx = 0u64;
        let mut tx = 0u64;
        let mut latency_us = 0u64;
        let mut conn_count = 0u64;
        let mut loss = 0f32;
        let mut tunnel = String::new();

        if let Some(peer) = &pair.peer {
            for conn in &peer.conns {
                if let Some(stats) = &conn.stats {
                    rx += stats.rx_bytes;
                    tx += stats.tx_bytes;
                    latency_us += stats.latency_us;
                    conn_count += 1;
                }
                loss = loss.max(conn.loss_rate);
                if tunnel.is_empty()
                    && let Some(t) = &conn.tunnel
                {
                    tunnel = t.tunnel_type.to_uppercase();
                }
            }
        }

        // Latency is only meaningful over a direct connection (averaged per-conn
        // RTT). Relayed peers have no direct connection, so leave it unknown
        // (rendered "—") instead of inventing a value — this mirrors
        // easytier-gui, which shows latency only when connection stats exist.
        // (`route.path_latency` is a routing *cost*, `cost % AVOID_RELAY_COST`,
        // not a latency — it reads as a bogus ~1000 for relayed peers.)
        let latency_ms = latency_us
            .checked_div(conn_count)
            .map(|us| us as f64 / 1000.0);

        net.rx_bytes += rx;
        net.tx_bytes += tx;

        net.peers.push(PeerRow {
            hostname: if route.hostname.is_empty() {
                format!("peer-{}", route.peer_id)
            } else {
                route.hostname.clone()
            },
            ipv4: route
                .ipv4_addr
                .as_ref()
                .map(|i| i.to_string())
                .unwrap_or_default(),
            cost: if route.cost <= 1 {
                "Direct".to_string()
            } else {
                format!("Relay ×{}", route.cost)
            },
            latency_ms,
            tunnel: if tunnel.is_empty() {
                "—".to_string()
            } else {
                tunnel
            },
            loss,
            rx,
            tx,
            nat: route
                .stun_info
                .as_ref()
                .map(|s| pretty_nat(s.udp_nat_type()))
                .unwrap_or_else(|| "Unknown".to_string()),
            version: if route.version.is_empty() {
                "—".to_string()
            } else {
                route.version.clone()
            },
        });
    }

    net.peers.sort_by_key(|a| a.hostname.to_lowercase());

    net
}

/// Map EasyTier's `NatType` debug repr to a friendlier label.
fn pretty_nat(n: easytier::proto::common::NatType) -> String {
    use easytier::proto::common::NatType::*;
    match n {
        Unknown => "Unknown",
        OpenInternet => "Open Internet",
        NoPat => "No PAT",
        FullCone => "Full Cone",
        Restricted => "Restricted",
        PortRestricted => "Port Restricted",
        Symmetric => "Symmetric",
        SymUdpFirewall => "Symmetric UDP Firewall",
        SymmetricEasyInc => "Symmetric Easy Inc",
        SymmetricEasyDec => "Symmetric Easy Dec",
    }
    .to_string()
}

/// EasyTier emits each event as JSON: `{"time": "...", "event": {<Variant>: ...}}`.
/// Render it as `HH:MM:SS  Variant  detail` and fall back to the raw text.
fn pretty_event(raw: &str) -> String {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return raw.to_string();
    };

    let time = v
        .get("time")
        .and_then(|t| t.as_str())
        .and_then(|t| t.get(11..19))
        .unwrap_or("")
        .to_string();

    let (name, detail) = match v.get("event") {
        Some(serde_json::Value::String(s)) => (s.clone(), String::new()),
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .next()
            .map(|(k, val)| (k.clone(), compact(val)))
            .unwrap_or_else(|| ("Event".to_string(), String::new())),
        _ => ("Event".to_string(), String::new()),
    };

    if detail.is_empty() {
        format!("{time}  {name}")
    } else {
        format!("{time}  {name}  {detail}")
    }
}

fn compact(v: &serde_json::Value) -> String {
    let s = v.to_string();
    match s.char_indices().nth(160) {
        Some((idx, _)) => format!("{}…", &s[..idx]),
        None => s,
    }
}

/// Human-readable byte size.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}
