//! App-side configuration: connection profiles and UI preferences, persisted as
//! JSON under `%APPDATA%\Polaris\config.json`, plus conversion into EasyTier's
//! `NetworkConfig`.
//!
//! Profile is a superset of every EasyTier `NetworkConfig` knob that the GUI
//! exposes — basic identity, addressing, transport flags, routing/whitelist,
//! VPN portal, port forwards, and the long tail of P2P / hole-punching /
//! encryption toggles. Each field has a sensible default so importing a
//! short EasyTier config (just `network_name` and `network_secret`) still
//! produces a working profile.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use easytier::common::config::{ConfigLoader, TomlConfigLoader};
use easytier::launcher::{NetworkConfig, NetworkingMethod};
use easytier::proto::api::manage::PortForwardConfig as PbPortForward;
use serde::{Deserialize, Serialize};

use crate::i18n::{Language, ts};

/// A process-unique, persisted id used to key a profile's running network.
fn gen_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{t:x}-{n:x}")
}

// ─────────────────────────────── Join method ──────────────────────────────

/// How this node joins the virtual network.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JoinMethod {
    /// Connect through a shared/public relay server.
    PublicServer,
    /// Connect to explicit peer URLs.
    Manual,
    /// Run a standalone node (others connect to us).
    Standalone,
}

impl JoinMethod {
    pub const ALL: [JoinMethod; 3] = [
        JoinMethod::PublicServer,
        JoinMethod::Manual,
        JoinMethod::Standalone,
    ];
    pub fn label(self) -> &'static str {
        ts(match self {
            JoinMethod::PublicServer => "join_method.public",
            JoinMethod::Manual => "join_method.manual",
            JoinMethod::Standalone => "join_method.standalone",
        })
    }
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as i32
    }
    pub fn from_index(i: i32) -> Self {
        *Self::ALL
            .get(i.max(0) as usize)
            .unwrap_or(&JoinMethod::PublicServer)
    }
}

// ────────────────────────────── Port forwarding ───────────────────────────

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortForward {
    pub proto: String, // "tcp" | "udp"
    pub bind_ip: String,
    pub bind_port: u16,
    pub dst_ip: String,
    pub dst_port: u16,
}

impl Default for PortForward {
    fn default() -> Self {
        PortForward {
            proto: "tcp".to_string(),
            bind_ip: "0.0.0.0".to_string(),
            bind_port: 0,
            dst_ip: "".to_string(),
            dst_port: 0,
        }
    }
}

impl PortForward {
    fn to_proto(&self) -> PbPortForward {
        PbPortForward {
            proto: self.proto.clone(),
            bind_ip: self.bind_ip.clone(),
            bind_port: self.bind_port as u32,
            dst_ip: self.dst_ip.clone(),
            dst_port: self.dst_port as u32,
        }
    }
    fn from_proto(pb: &PbPortForward) -> Self {
        PortForward {
            proto: if pb.proto.is_empty() {
                "tcp".to_string()
            } else {
                pb.proto.clone()
            },
            bind_ip: pb.bind_ip.clone(),
            bind_port: pb.bind_port as u16,
            dst_ip: pb.dst_ip.clone(),
            dst_port: pb.dst_port as u16,
        }
    }
}

// ─────────────────────────────── Profile ──────────────────────────────────

/// A single, named connection profile — superset of EasyTier's `NetworkConfig`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    #[serde(default = "gen_id")]
    pub id: String,
    pub name: String,

    // Identity
    pub network_name: String,
    pub network_secret: String,
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub dev_name: String,

    // Addressing
    pub dhcp: bool,
    pub virtual_ipv4: String,
    #[serde(default = "default_network_length")]
    pub network_length: u8,

    // Connection
    pub join_method: JoinMethod,
    pub public_server: String,
    pub peers: Vec<String>,
    pub listeners: Vec<String>,
    #[serde(default)]
    pub mapped_listeners: Vec<String>,

    // Subnet proxy / SOCKS5
    pub proxy_cidrs: Vec<String>,
    pub enable_socks5: bool,
    pub socks5_port: u16,

    // VPN Portal (built-in WireGuard server for outside clients).
    #[serde(default)]
    pub enable_vpn_portal: bool,
    #[serde(default = "default_vpn_portal_port")]
    pub vpn_portal_listen_port: u16,
    #[serde(default = "default_vpn_portal_cidr")]
    pub vpn_portal_client_network_addr: String,
    #[serde(default = "default_vpn_portal_len")]
    pub vpn_portal_client_network_len: u8,

    // Routing
    #[serde(default)]
    pub enable_manual_routes: bool,
    #[serde(default)]
    pub routes: Vec<String>,
    #[serde(default)]
    pub exit_nodes: Vec<String>,
    #[serde(default)]
    pub enable_exit_node: bool,
    #[serde(default)]
    pub enable_relay_network_whitelist: bool,
    #[serde(default)]
    pub relay_network_whitelist: Vec<String>,

    // Port forwarding
    #[serde(default)]
    pub port_forwards: Vec<PortForward>,

    // Performance limits (0 = unset / default).
    #[serde(default)]
    pub mtu: u32,
    #[serde(default)]
    pub instance_recv_bps_limit: u64,

    // Headline flags.
    pub latency_first: bool,
    pub enable_encryption: bool,

    // Transport flags.
    #[serde(default)]
    pub use_smoltcp: bool,
    #[serde(default)]
    pub disable_ipv6: bool,
    #[serde(default)]
    pub ipv6_public_addr_auto: bool,
    #[serde(default)]
    pub enable_kcp_proxy: bool,
    #[serde(default)]
    pub disable_kcp_input: bool,
    #[serde(default)]
    pub enable_quic_proxy: bool,
    #[serde(default)]
    pub disable_quic_input: bool,
    #[serde(default)]
    pub enable_udp_broadcast_relay: bool,

    // P2P / hole punching.
    #[serde(default)]
    pub disable_p2p: bool,
    #[serde(default)]
    pub p2p_only: bool,
    #[serde(default)]
    pub lazy_p2p: bool,
    #[serde(default)]
    pub need_p2p: bool,
    #[serde(default)]
    pub disable_tcp_hole_punching: bool,
    #[serde(default)]
    pub disable_udp_hole_punching: bool,
    #[serde(default)]
    pub disable_sym_hole_punching: bool,
    #[serde(default)]
    pub disable_upnp: bool,

    // Routing / exit / instance.
    #[serde(default)]
    pub relay_all_peer_rpc: bool,
    #[serde(default = "default_true")]
    pub multi_thread: bool,
    #[serde(default)]
    pub proxy_forward_by_system: bool,
    #[serde(default = "default_true")]
    pub bind_device: bool,
    #[serde(default)]
    pub no_tun: bool,

    // Privacy / DNS.
    #[serde(default)]
    pub enable_magic_dns: bool,
    #[serde(default)]
    pub enable_private_mode: bool,
}

fn default_true() -> bool {
    true
}
fn default_log_level() -> LogLevel {
    LogLevel::Info
}
fn default_log_retention() -> u32 {
    7
}
fn default_network_length() -> u8 {
    24
}
fn default_vpn_portal_port() -> u16 {
    22022
}
fn default_vpn_portal_cidr() -> String {
    "10.14.14.0".to_string()
}
fn default_vpn_portal_len() -> u8 {
    24
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            id: gen_id(),
            name: "My network".to_string(),
            network_name: "polaris-net".to_string(),
            network_secret: "change-this-secret".to_string(),
            hostname: String::new(),
            dev_name: String::new(),
            dhcp: true,
            virtual_ipv4: "10.126.126.1".to_string(),
            network_length: 24,
            join_method: JoinMethod::PublicServer,
            public_server: "tcp://public.easytier.cn:11010".to_string(),
            peers: Vec::new(),
            listeners: Vec::new(),
            mapped_listeners: Vec::new(),
            proxy_cidrs: Vec::new(),
            enable_socks5: false,
            socks5_port: 1080,
            enable_vpn_portal: false,
            vpn_portal_listen_port: default_vpn_portal_port(),
            vpn_portal_client_network_addr: default_vpn_portal_cidr(),
            vpn_portal_client_network_len: default_vpn_portal_len(),
            enable_manual_routes: false,
            routes: Vec::new(),
            exit_nodes: Vec::new(),
            enable_exit_node: false,
            enable_relay_network_whitelist: false,
            relay_network_whitelist: Vec::new(),
            port_forwards: Vec::new(),
            mtu: 0,
            instance_recv_bps_limit: 0,
            latency_first: true,
            enable_encryption: true,
            use_smoltcp: false,
            disable_ipv6: false,
            ipv6_public_addr_auto: false,
            enable_kcp_proxy: false,
            disable_kcp_input: false,
            enable_quic_proxy: false,
            disable_quic_input: false,
            enable_udp_broadcast_relay: false,
            disable_p2p: false,
            p2p_only: false,
            lazy_p2p: false,
            need_p2p: false,
            disable_tcp_hole_punching: false,
            disable_udp_hole_punching: false,
            disable_sym_hole_punching: false,
            disable_upnp: false,
            relay_all_peer_rpc: false,
            multi_thread: true,
            proxy_forward_by_system: false,
            bind_device: true,
            no_tun: false,
            enable_magic_dns: false,
            enable_private_mode: false,
        }
    }
}

impl Profile {
    /// Translate into EasyTier's launcher config. `gen_config()` fills sensible
    /// defaults (random instance id, default listeners) for anything left unset.
    pub fn to_network_config(&self) -> NetworkConfig {
        let trimmed = |s: &str| {
            let t = s.trim().to_string();
            (!t.is_empty()).then_some(t)
        };

        NetworkConfig {
            // Identity.
            network_name: Some(self.network_name.trim().to_string()),
            network_secret: Some(self.network_secret.clone()),
            hostname: trimmed(&self.hostname),
            dev_name: trimmed(&self.dev_name),

            // Addressing.
            dhcp: Some(self.dhcp),
            virtual_ipv4: (!self.dhcp).then(|| self.virtual_ipv4.trim().to_string()),
            network_length: (!self.dhcp).then_some(self.network_length as i32),

            // Connection.
            networking_method: Some(match self.join_method {
                JoinMethod::PublicServer => NetworkingMethod::PublicServer as i32,
                JoinMethod::Manual => NetworkingMethod::Manual as i32,
                JoinMethod::Standalone => NetworkingMethod::Standalone as i32,
            }),
            public_server_url: trimmed(&self.public_server),
            peer_urls: clean_list(&self.peers),
            listener_urls: clean_list(&self.listeners),
            mapped_listeners: clean_list(&self.mapped_listeners),

            // Subnet proxy / SOCKS5.
            proxy_cidrs: clean_list(&self.proxy_cidrs),
            enable_socks5: Some(self.enable_socks5),
            socks5_port: self.enable_socks5.then_some(self.socks5_port as i32),

            // VPN portal.
            enable_vpn_portal: Some(self.enable_vpn_portal),
            vpn_portal_listen_port: self
                .enable_vpn_portal
                .then_some(self.vpn_portal_listen_port as i32),
            vpn_portal_client_network_addr: self
                .enable_vpn_portal
                .then(|| self.vpn_portal_client_network_addr.trim().to_string()),
            vpn_portal_client_network_len: self
                .enable_vpn_portal
                .then_some(self.vpn_portal_client_network_len as i32),

            // Routing.
            enable_manual_routes: Some(self.enable_manual_routes),
            routes: clean_list(&self.routes),
            exit_nodes: clean_list(&self.exit_nodes),
            enable_exit_node: Some(self.enable_exit_node),
            enable_relay_network_whitelist: Some(self.enable_relay_network_whitelist),
            relay_network_whitelist: clean_list(&self.relay_network_whitelist),

            // Port forwarding.
            port_forwards: self
                .port_forwards
                .iter()
                .filter(|f| f.bind_port != 0 && f.dst_port != 0 && !f.dst_ip.trim().is_empty())
                .map(|f| f.to_proto())
                .collect(),

            // Limits.
            mtu: (self.mtu > 0).then_some(self.mtu as i32),
            instance_recv_bps_limit: (self.instance_recv_bps_limit > 0)
                .then_some(self.instance_recv_bps_limit),

            // Headline.
            latency_first: Some(self.latency_first),
            disable_encryption: Some(!self.enable_encryption),

            // Transport.
            use_smoltcp: Some(self.use_smoltcp),
            disable_ipv6: Some(self.disable_ipv6),
            ipv6_public_addr_auto: Some(self.ipv6_public_addr_auto),
            enable_kcp_proxy: Some(self.enable_kcp_proxy),
            disable_kcp_input: Some(self.disable_kcp_input),
            enable_quic_proxy: Some(self.enable_quic_proxy),
            disable_quic_input: Some(self.disable_quic_input),
            enable_udp_broadcast_relay: Some(self.enable_udp_broadcast_relay),

            // P2P / hole-punching.
            disable_p2p: Some(self.disable_p2p),
            p2p_only: Some(self.p2p_only),
            lazy_p2p: Some(self.lazy_p2p),
            need_p2p: Some(self.need_p2p),
            disable_tcp_hole_punching: Some(self.disable_tcp_hole_punching),
            disable_udp_hole_punching: Some(self.disable_udp_hole_punching),
            disable_sym_hole_punching: Some(self.disable_sym_hole_punching),
            disable_upnp: Some(self.disable_upnp),

            // Routing / instance.
            relay_all_peer_rpc: Some(self.relay_all_peer_rpc),
            multi_thread: Some(self.multi_thread),
            proxy_forward_by_system: Some(self.proxy_forward_by_system),
            bind_device: Some(self.bind_device),
            no_tun: Some(self.no_tun),

            // Privacy / DNS.
            enable_magic_dns: Some(self.enable_magic_dns),
            enable_private_mode: Some(self.enable_private_mode),

            ..NetworkConfig::default()
        }
    }

    /// Build a profile from an EasyTier `NetworkConfig` (an imported config
    /// shared with the official EasyTier GUI). A fresh local id is assigned.
    // Copies 40+ fields out of `nc`; default-then-assign is far less noisy here
    // than a struct literal.
    #[allow(clippy::field_reassign_with_default)]
    pub fn from_network_config(nc: &NetworkConfig) -> Profile {
        let net_name = nc.network_name.clone().unwrap_or_default();
        let mut p = Profile::default();
        p.name = if net_name.is_empty() {
            "Imported network".to_string()
        } else {
            net_name.clone()
        };
        p.network_name = net_name;
        p.network_secret = nc.network_secret.clone().unwrap_or_default();
        p.hostname = nc.hostname.clone().unwrap_or_default();
        p.dev_name = nc.dev_name.clone().unwrap_or_default();

        p.dhcp = nc.dhcp.unwrap_or(true);
        p.virtual_ipv4 = nc.virtual_ipv4.clone().unwrap_or_default();
        if let Some(len) = nc.network_length
            && len > 0
        {
            p.network_length = len.clamp(1, 32) as u8;
        }

        p.join_method = JoinMethod::from_index(nc.networking_method.unwrap_or(0));
        p.public_server = nc.public_server_url.clone().unwrap_or_default();
        p.peers = nc.peer_urls.clone();
        p.listeners = nc.listener_urls.clone();
        p.mapped_listeners = nc.mapped_listeners.clone();

        p.proxy_cidrs = nc.proxy_cidrs.clone();
        p.enable_socks5 = nc.enable_socks5.unwrap_or(false);
        if let Some(port) = nc.socks5_port
            && port > 0
        {
            p.socks5_port = port as u16;
        }

        p.enable_vpn_portal = nc.enable_vpn_portal.unwrap_or(false);
        if let Some(port) = nc.vpn_portal_listen_port
            && port > 0
        {
            p.vpn_portal_listen_port = port as u16;
        }
        if let Some(cidr) = &nc.vpn_portal_client_network_addr
            && !cidr.is_empty()
        {
            p.vpn_portal_client_network_addr = cidr.clone();
        }
        if let Some(len) = nc.vpn_portal_client_network_len
            && len > 0
        {
            p.vpn_portal_client_network_len = len.clamp(1, 32) as u8;
        }

        p.enable_manual_routes = nc.enable_manual_routes.unwrap_or(false);
        p.routes = nc.routes.clone();
        p.exit_nodes = nc.exit_nodes.clone();
        p.enable_exit_node = nc.enable_exit_node.unwrap_or(false);
        p.enable_relay_network_whitelist = nc.enable_relay_network_whitelist.unwrap_or(false);
        p.relay_network_whitelist = nc.relay_network_whitelist.clone();

        p.port_forwards = nc
            .port_forwards
            .iter()
            .map(PortForward::from_proto)
            .collect();

        p.mtu = nc.mtu.unwrap_or(0).max(0) as u32;
        p.instance_recv_bps_limit = nc.instance_recv_bps_limit.unwrap_or(0);

        p.latency_first = nc.latency_first.unwrap_or(true);
        p.enable_encryption = !nc.disable_encryption.unwrap_or(false);

        p.use_smoltcp = nc.use_smoltcp.unwrap_or(false);
        p.disable_ipv6 = nc.disable_ipv6.unwrap_or(false);
        p.ipv6_public_addr_auto = nc.ipv6_public_addr_auto.unwrap_or(false);
        p.enable_kcp_proxy = nc.enable_kcp_proxy.unwrap_or(false);
        p.disable_kcp_input = nc.disable_kcp_input.unwrap_or(false);
        p.enable_quic_proxy = nc.enable_quic_proxy.unwrap_or(false);
        p.disable_quic_input = nc.disable_quic_input.unwrap_or(false);
        p.enable_udp_broadcast_relay = nc.enable_udp_broadcast_relay.unwrap_or(false);

        p.disable_p2p = nc.disable_p2p.unwrap_or(false);
        p.p2p_only = nc.p2p_only.unwrap_or(false);
        p.lazy_p2p = nc.lazy_p2p.unwrap_or(false);
        p.need_p2p = nc.need_p2p.unwrap_or(false);
        p.disable_tcp_hole_punching = nc.disable_tcp_hole_punching.unwrap_or(false);
        p.disable_udp_hole_punching = nc.disable_udp_hole_punching.unwrap_or(false);
        p.disable_sym_hole_punching = nc.disable_sym_hole_punching.unwrap_or(false);
        p.disable_upnp = nc.disable_upnp.unwrap_or(false);

        p.relay_all_peer_rpc = nc.relay_all_peer_rpc.unwrap_or(false);
        p.multi_thread = nc.multi_thread.unwrap_or(true);
        p.proxy_forward_by_system = nc.proxy_forward_by_system.unwrap_or(false);
        p.bind_device = nc.bind_device.unwrap_or(true);
        p.no_tun = nc.no_tun.unwrap_or(false);

        p.enable_magic_dns = nc.enable_magic_dns.unwrap_or(false);
        p.enable_private_mode = nc.enable_private_mode.unwrap_or(false);

        p
    }
}

/// Serialize a profile to EasyTier's canonical TOML config (what
/// `easytier-core -c` and the official GUI's "export" produce).
pub fn profile_to_toml(p: &Profile) -> anyhow::Result<String> {
    Ok(p.to_network_config().gen_config()?.dump())
}

/// Serialize a profile to the EasyTier `NetworkConfig` JSON used by the GUI.
pub fn profile_to_json(p: &Profile) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(&p.to_network_config())?)
}

/// Serialize a profile to Polaris's own JSON. Lossless — unlike the EasyTier
/// formats (which carry only the `NetworkConfig` subset), this keeps the display
/// name, id, and every Polaris-only field, so it re-imports exactly.
pub fn profile_to_polaris_json(p: &Profile) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(p)?)
}

/// Serialize the whole store — every network plus the app settings — to a backup
/// JSON. Same shape as the on-disk `config.json`; restore with [`parse_backup`].
pub fn store_to_backup_json(store: &Store) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(store)?)
}

/// Parse a full backup (a [`Store`]) for "Restore backup". Profile ids are
/// re-keyed so a restored network can't collide with one still running under an
/// old id.
pub fn parse_backup(path: &Path) -> anyhow::Result<Store> {
    let text = std::fs::read_to_string(path)?;
    let mut store: Store =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("not a Polaris backup: {e}"))?;
    if store.profiles.is_empty() {
        anyhow::bail!("backup contains no networks");
    }
    for p in &mut store.profiles {
        p.id = gen_id();
    }
    store.selected = store.selected.min(store.profiles.len() - 1);
    Ok(store)
}

/// Parse one or more profiles from a `.toml` or `.json` file produced by either
/// Polaris or the official EasyTier GUI.
pub fn import_profiles(path: &Path) -> anyhow::Result<Vec<Profile>> {
    let text = std::fs::read_to_string(path)?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "json" {
        return parse_json_configs(&text);
    }

    parse_toml_config(&text)
}

/// Parse profiles from raw config text (pasted by the user), auto-detecting
/// JSON vs the EasyTier TOML format.
pub fn parse_profiles_from_text(text: &str) -> anyhow::Result<Vec<Profile>> {
    if text.trim().is_empty() {
        anyhow::bail!("clipboard is empty");
    }
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_json_configs(text)
    } else {
        parse_toml_config(text)
    }
}

fn parse_toml_config(text: &str) -> anyhow::Result<Vec<Profile>> {
    let loader = TomlConfigLoader::new_from_str(text)
        .map_err(|e| anyhow::anyhow!("not a valid EasyTier config: {e}"))?;
    let nc = NetworkConfig::new_from_config(&loader)?;
    Ok(vec![Profile::from_network_config(&nc)])
}

fn parse_json_configs(text: &str) -> anyhow::Result<Vec<Profile>> {
    // Polaris-native exports first. EasyTier's NetworkConfig JSON lacks Profile's
    // required fields (name, join_method, peers, …), so it can't match these by
    // accident; a full backup (Store) contributes just its networks. Imported
    // copies are re-keyed (import = add new networks, never adopt a live id).
    if let Ok(mut p) = serde_json::from_str::<Profile>(text) {
        p.id = gen_id();
        return Ok(vec![p]);
    }
    if let Ok(v) = serde_json::from_str::<Vec<Profile>>(text)
        && !v.is_empty()
    {
        return Ok(v
            .into_iter()
            .map(|mut p| {
                p.id = gen_id();
                p
            })
            .collect());
    }
    if let Ok(store) = serde_json::from_str::<Store>(text)
        && !store.profiles.is_empty()
    {
        return Ok(store
            .profiles
            .into_iter()
            .map(|mut p| {
                p.id = gen_id();
                p
            })
            .collect());
    }

    #[derive(Deserialize)]
    struct Stored {
        config: NetworkConfig,
    }
    // The official GUI stores `[{ "config": NetworkConfig, ... }]`.
    if let Ok(arr) = serde_json::from_str::<Vec<Stored>>(text)
        && !arr.is_empty()
    {
        return Ok(arr
            .iter()
            .map(|s| Profile::from_network_config(&s.config))
            .collect());
    }
    if let Ok(arr) = serde_json::from_str::<Vec<NetworkConfig>>(text)
        && !arr.is_empty()
    {
        return Ok(arr.iter().map(Profile::from_network_config).collect());
    }
    let nc: NetworkConfig = serde_json::from_str(text)?;
    Ok(vec![Profile::from_network_config(&nc)])
}

fn clean_list(items: &[String]) -> Vec<String> {
    items
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ─────────────────────────── Settings & Store ─────────────────────────────

/// Application appearance theme.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::System, Theme::Light, Theme::Dark];
    pub fn label(self) -> &'static str {
        ts(match self {
            Theme::System => "theme.system",
            Theme::Light => "theme.light",
            Theme::Dark => "theme.dark",
        })
    }
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as i32
    }
    pub fn from_index(i: i32) -> Self {
        *Self::ALL.get(i.max(0) as usize).unwrap_or(&Theme::System)
    }
}

/// Window backdrop material.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Material {
    #[default]
    Mica,
    MicaAlt,
    Acrylic,
    Solid,
}

impl Material {
    pub const ALL: [Material; 4] = [
        Material::Mica,
        Material::MicaAlt,
        Material::Acrylic,
        Material::Solid,
    ];
    pub fn label(self) -> &'static str {
        ts(match self {
            Material::Mica => "material.mica",
            Material::MicaAlt => "material.mica_alt",
            Material::Acrylic => "material.acrylic",
            Material::Solid => "material.solid",
        })
    }
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0) as i32
    }
    pub fn from_index(i: i32) -> Self {
        *Self::ALL.get(i.max(0) as usize).unwrap_or(&Material::Mica)
    }
}

/// Diagnostics verbosity for the in-app log panel and on-disk log files.
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub const ALL: [LogLevel; 6] = [
        LogLevel::Off,
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];
    pub fn label(self) -> &'static str {
        ts(match self {
            LogLevel::Off => "log_level.off",
            LogLevel::Error => "log_level.error",
            LogLevel::Warn => "log_level.warn",
            LogLevel::Info => "log_level.info",
            LogLevel::Debug => "log_level.debug",
            LogLevel::Trace => "log_level.trace",
        })
    }
    /// `tracing` filter directive token.
    pub fn as_filter(self) -> &'static str {
        match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
    pub fn index(self) -> i32 {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(3) as i32
    }
    pub fn from_index(i: i32) -> Self {
        *Self::ALL.get(i.max(0) as usize).unwrap_or(&LogLevel::Info)
    }
}

/// UI preferences.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub theme: Theme,
    pub material: Material,
    pub tall_titlebar: bool,
    pub auto_connect: bool,
    /// Closing the window hides Polaris to the tray instead of quitting.
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// Minimizing the window hides Polaris to the tray instead of the taskbar.
    #[serde(default)]
    pub minimize_to_tray: bool,
    /// Request UAC elevation on launch so the VPN (TUN) adapter can be created.
    #[serde(default)]
    pub always_admin: bool,
    /// Collect logs and surface the Diagnostics panel in the sidebar.
    #[serde(default)]
    pub diagnostics_enabled: bool,
    /// Log verbosity while diagnostics are enabled.
    #[serde(default = "default_log_level")]
    pub log_level: LogLevel,
    /// Delete log files older than this many days (run on startup).
    #[serde(default = "default_log_retention")]
    pub log_retention_days: u32,
    /// UI language; `System` follows the OS locale (English unless Chinese).
    #[serde(default)]
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: Theme::System,
            material: Material::Mica,
            tall_titlebar: true,
            auto_connect: false,
            close_to_tray: true,
            minimize_to_tray: false,
            always_admin: false,
            diagnostics_enabled: false,
            log_level: LogLevel::Info,
            log_retention_days: 7,
            language: Language::System,
        }
    }
}

/// Everything we persist.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Store {
    pub profiles: Vec<Profile>,
    pub selected: usize,
    pub settings: Settings,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            profiles: vec![Profile::default()],
            selected: 0,
            settings: Settings::default(),
        }
    }
}

impl Store {
    fn path() -> Option<PathBuf> {
        let mut p = dirs::config_dir()?;
        p.push("Polaris");
        p.push("config.json");
        Some(p)
    }

    /// Load from disk, falling back to defaults on any error.
    pub fn load() -> Store {
        let Some(path) = Self::path() else {
            return Store::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Store::default();
        };
        let mut store: Store = serde_json::from_str(&text).unwrap_or_default();
        if store.profiles.is_empty() {
            store.profiles.push(Profile::default());
        }
        store.selected = store.selected.min(store.profiles.len() - 1);
        store
    }

    /// Persist to disk (best effort).
    pub fn save(&self) {
        let Some(path) = Self::path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }

    pub fn current(&self) -> &Profile {
        &self.profiles[self.selected.min(self.profiles.len() - 1)]
    }
}
