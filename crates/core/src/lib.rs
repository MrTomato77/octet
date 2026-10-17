//! Pure IPv4 draft validation and change-planning rules. No operating-system mutation APIs.

use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpConfig {
    Dhcp,
    Static {
        address: Ipv4Addr,
        prefix: u8,
        gateway: Option<Ipv4Addr>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsConfig {
    Automatic,
    Manual(Vec<Ipv4Addr>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub ip: IpConfig,
    pub dns: DnsConfig,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            ip: IpConfig::Dhcp,
            dns: DnsConfig::Automatic,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorDraft {
    pub automatic_ip: bool,
    pub address: String,
    pub subnet: String,
    pub gateway: String,
    pub automatic_dns: bool,
    pub preferred_dns: String,
    pub alternate_dns: String,
}

impl EditorDraft {
    pub fn from_config(config: &NetworkConfig) -> Self {
        let (automatic_ip, address, subnet, gateway) = match config.ip {
            IpConfig::Dhcp => (true, String::new(), "255.255.255.0".into(), String::new()),
            IpConfig::Static {
                address,
                prefix,
                gateway,
            } => (
                false,
                address.to_string(),
                prefix_mask(prefix).to_string(),
                gateway.map(|ip| ip.to_string()).unwrap_or_default(),
            ),
        };
        let (automatic_dns, preferred_dns, alternate_dns) = match &config.dns {
            DnsConfig::Automatic => (true, String::new(), String::new()),
            DnsConfig::Manual(servers) => (
                false,
                servers.first().map(ToString::to_string).unwrap_or_default(),
                servers.get(1).map(ToString::to_string).unwrap_or_default(),
            ),
        };
        Self {
            automatic_ip,
            address,
            subnet,
            gateway,
            automatic_dns,
            preferred_dns,
            alternate_dns,
        }
    }

    pub fn validate(&self) -> Result<NetworkConfig, Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        let ip = if self.automatic_ip {
            Some(IpConfig::Dhcp)
        } else {
            let address = parse_host(&self.address, "IP address", &mut issues);
            let prefix = match parse_subnet(&self.subnet) {
                Ok(prefix) => Some(prefix),
                Err(message) => {
                    issues.push(ValidationIssue::new("Subnet mask", message));
                    None
                }
            };
            let gateway = if self.gateway.trim().is_empty() {
                None
            } else {
                parse_host(&self.gateway, "Gateway", &mut issues)
            };
            if let (Some(address), Some(prefix)) = (address, prefix) {
                let mask = u32::from(prefix_mask(prefix));
                let host = u32::from(address) & !mask;
                if prefix < 31 && (host == 0 || host == !mask) {
                    issues.push(ValidationIssue::new(
                        "IP address",
                        "Use a host address, not the subnet or broadcast address.",
                    ));
                }
                if let Some(gateway) = gateway {
                    if gateway == address {
                        issues.push(ValidationIssue::new(
                            "Gateway",
                            "Gateway must differ from the IP address.",
                        ));
                    } else if u32::from(gateway) & mask != u32::from(address) & mask {
                        issues.push(ValidationIssue::new(
                            "Gateway",
                            "Gateway must be in the same subnet in this version.",
                        ));
                    } else if prefix < 31 {
                        let gateway_host = u32::from(gateway) & !mask;
                        if gateway_host == 0 || gateway_host == !mask {
                            issues.push(ValidationIssue::new(
                                "Gateway",
                                "Gateway must be a host address.",
                            ));
                        }
                    }
                }
                Some(IpConfig::Static {
                    address,
                    prefix,
                    gateway,
                })
            } else {
                None
            }
        };
        let dns = if self.automatic_dns {
            if !self.automatic_ip {
                issues.push(ValidationIssue::new(
                    "DNS",
                    "Select manual DNS for a static IP configuration.",
                ));
            }
            Some(DnsConfig::Automatic)
        } else {
            let mut servers = Vec::new();
            for (value, field) in [
                (&self.preferred_dns, "Preferred DNS"),
                (&self.alternate_dns, "Alternate DNS"),
            ] {
                if !value.trim().is_empty()
                    && let Some(address) = parse_dns(value, field, &mut issues)
                {
                    if servers.contains(&address) {
                        issues.push(ValidationIssue::new(
                            field,
                            "Use a different DNS server, or leave this field empty.",
                        ));
                    } else {
                        servers.push(address);
                    }
                }
            }
            if servers.is_empty()
                && self.preferred_dns.trim().is_empty()
                && self.alternate_dns.trim().is_empty()
            {
                issues.push(ValidationIssue::new(
                    "DNS",
                    "Enter at least one DNS server. Empty fields do not mean automatic DNS.",
                ));
            }
            Some(DnsConfig::Manual(servers))
        };
        if issues.is_empty() {
            match (ip, dns) {
                (Some(ip), Some(dns)) => Ok(NetworkConfig { ip, dns }),
                _ => Err(vec![ValidationIssue::new(
                    "Settings",
                    "The configuration is incomplete.",
                )]),
            }
        } else {
            Err(issues)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationIssue {
    pub field: &'static str,
    pub message: &'static str,
}

impl ValidationIssue {
    fn new(field: &'static str, message: &'static str) -> Self {
        Self { field, message }
    }
}

fn parse_host(
    value: &str,
    field: &'static str,
    issues: &mut Vec<ValidationIssue>,
) -> Option<Ipv4Addr> {
    match value.trim().parse::<Ipv4Addr>() {
        Ok(address)
            if !address.is_unspecified()
                && !address.is_loopback()
                && !address.is_multicast()
                && !address.is_broadcast()
                && address.octets()[0] != 0
                && address.octets()[0] < 240 =>
        {
            Some(address)
        }
        _ => {
            issues.push(ValidationIssue::new(
                field,
                "Enter a usable IPv4 address, for example 192.0.2.10.",
            ));
            None
        }
    }
}

fn parse_dns(
    value: &str,
    field: &'static str,
    issues: &mut Vec<ValidationIssue>,
) -> Option<Ipv4Addr> {
    match value.trim().parse::<Ipv4Addr>() {
        Ok(address)
            if !address.is_unspecified()
                && !address.is_multicast()
                && !address.is_broadcast()
                && address.octets()[0] != 0
                && address.octets()[0] < 240 =>
        {
            Some(address)
        }
        _ => {
            issues.push(ValidationIssue::new(
                field,
                "Enter an IPv4 DNS server address.",
            ));
            None
        }
    }
}

pub fn prefix_mask(prefix: u8) -> Ipv4Addr {
    Ipv4Addr::from(
        u32::MAX
            .checked_shl(32 - u32::from(prefix.min(32)))
            .unwrap_or(0),
    )
}

pub fn parse_subnet(value: &str) -> Result<u8, &'static str> {
    let value = value.trim().trim_start_matches('/');
    if let Ok(prefix) = value.parse::<u8>() {
        return (prefix <= 32)
            .then_some(prefix)
            .ok_or("Prefix must be between 0 and 32.");
    }
    let mask = value
        .parse::<Ipv4Addr>()
        .map(u32::from)
        .map_err(|_| "Enter a subnet mask or a prefix from 0 to 32.")?;
    let prefix = mask.leading_ones() as u8;
    if u32::from(prefix_mask(prefix)) == mask {
        Ok(prefix)
    } else {
        Err("Subnet mask bits must be contiguous.")
    }
}

#[derive(Clone, Debug)]
pub struct SampleAdapter {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub mac: String,
    pub connected: bool,
    pub address: Option<Ipv4Addr>,
    pub prefix: u8,
    pub gateway: Option<Ipv4Addr>,
    pub dns: Vec<Ipv4Addr>,
    pub dhcp_server: Option<Ipv4Addr>,
    pub config: NetworkConfig,
}

fn ip(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
    Ipv4Addr::new(a, b, c, d)
}

pub fn sample_adapters() -> Vec<SampleAdapter> {
    vec![
        SampleAdapter {
            id: Uuid::from_u128(1),
            name: "Ethernet".into(),
            description: "Sample Ethernet adapter".into(),
            mac: "02-00-00-00-00-01".into(),
            connected: true,
            address: Some(ip(192, 0, 2, 24)),
            prefix: 24,
            gateway: Some(ip(192, 0, 2, 1)),
            dns: vec![ip(192, 0, 2, 53)],
            dhcp_server: Some(ip(192, 0, 2, 1)),
            config: NetworkConfig::default(),
        },
        SampleAdapter {
            id: Uuid::from_u128(2),
            name: "Wi-Fi".into(),
            description: "Sample wireless adapter".into(),
            mac: "02-00-00-00-00-02".into(),
            connected: false,
            address: None,
            prefix: 0,
            gateway: None,
            dns: vec![],
            dhcp_server: None,
            config: NetworkConfig::default(),
        },
        SampleAdapter {
            id: Uuid::from_u128(3),
            name: "USB Ethernet".into(),
            description: "Sample laboratory adapter".into(),
            mac: "02-00-00-00-00-03".into(),
            connected: true,
            address: Some(ip(198, 51, 100, 8)),
            prefix: 24,
            gateway: None,
            dns: vec![ip(198, 51, 100, 53)],
            dhcp_server: None,
            config: NetworkConfig {
                ip: IpConfig::Static {
                    address: ip(198, 51, 100, 8),
                    prefix: 24,
                    gateway: None,
                },
                dns: DnsConfig::Manual(vec![ip(198, 51, 100, 53)]),
            },
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub field: &'static str,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug)]
pub struct ChangePlan {
    pub target_id: Uuid,
    pub target_name: String,
    pub changes: Vec<Change>,
}

impl ChangePlan {
    pub fn preview(adapter: &SampleAdapter, desired: &NetworkConfig) -> Self {
        let mut changes = Vec::new();
        let mut add = |field, before: String, after: String| {
            if before != after {
                changes.push(Change {
                    field,
                    before,
                    after,
                });
            }
        };
        let optional = |ip: Option<Ipv4Addr>| {
            ip.map(|v| v.to_string())
                .unwrap_or_else(|| "Not assigned".into())
        };
        let mode = |ip: &IpConfig| {
            if matches!(ip, IpConfig::Dhcp) {
                "DHCP"
            } else {
                "Manual"
            }
            .to_owned()
        };
        add("IP mode", mode(&adapter.config.ip), mode(&desired.ip));
        if let IpConfig::Static {
            address,
            prefix,
            gateway,
        } = desired.ip
        {
            add("IP address", optional(adapter.address), address.to_string());
            add(
                "Subnet / prefix",
                if adapter.address.is_some() {
                    format!("/{}", adapter.prefix)
                } else {
                    "Not assigned".into()
                },
                format!("/{prefix}"),
            );
            add("Gateway", optional(adapter.gateway), optional(gateway));
        }
        let mode = |dns: &DnsConfig| {
            if matches!(dns, DnsConfig::Automatic) {
                "Automatic"
            } else {
                "Manual"
            }
            .to_owned()
        };
        add("DNS mode", mode(&adapter.config.dns), mode(&desired.dns));
        if let DnsConfig::Manual(servers) = &desired.dns {
            for (index, field) in [(0, "Preferred DNS"), (1, "Alternate DNS")] {
                add(
                    field,
                    optional(adapter.dns.get(index).copied()),
                    optional(servers.get(index).copied()),
                );
            }
        }
        Self {
            target_id: adapter.id,
            target_name: adapter.name.clone(),
            changes,
        }
    }
}

pub fn describe_ip(config: &IpConfig) -> String {
    match config {
        IpConfig::Dhcp => "Automatic (DHCP)".into(),
        IpConfig::Static {
            address,
            prefix,
            gateway,
        } => format!(
            "{address}/{prefix}; gateway {}",
            gateway
                .map(|v| v.to_string())
                .unwrap_or_else(|| "none".into())
        ),
    }
}

pub fn describe_dns(config: &DnsConfig) -> String {
    match config {
        DnsConfig::Automatic => "Automatic (DHCP)".into(),
        DnsConfig::Manual(servers) => servers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_draft() -> EditorDraft {
        EditorDraft::from_config(&NetworkConfig {
            ip: IpConfig::Static {
                address: ip(198, 51, 100, 10),
                prefix: 24,
                gateway: None,
            },
            dns: DnsConfig::Manual(vec![ip(198, 51, 100, 53)]),
        })
    }

    #[test]
    fn subnet_masks_require_contiguous_bits_and_handle_edges() {
        assert_eq!(parse_subnet("255.255.255.0"), Ok(24));
        assert_eq!(parse_subnet("/31"), Ok(31));
        assert_eq!(parse_subnet("32"), Ok(32));
        assert_eq!(parse_subnet("0.0.0.0"), Ok(0));
        assert!(parse_subnet("255.0.255.0").is_err());
        assert!(parse_subnet("33").is_err());
        assert_eq!(prefix_mask(32), Ipv4Addr::BROADCAST);
    }

    #[test]
    fn alternate_only_dns_becomes_the_first_typed_server() {
        let mut draft = static_draft();
        draft.preferred_dns.clear();
        draft.alternate_dns = "192.0.2.53".into();
        assert_eq!(
            draft.validate().unwrap().dns,
            DnsConfig::Manual(vec![ip(192, 0, 2, 53)])
        );
    }

    #[test]
    fn dhcp_ignores_hidden_invalid_static_fields() {
        let mut draft = static_draft();
        draft.automatic_ip = true;
        draft.address = "not an address".into();
        draft.subnet = "invalid".into();
        assert_eq!(draft.validate().unwrap().ip, IpConfig::Dhcp);
    }

    #[test]
    fn static_rejects_auto_dns_and_wrong_gateway() {
        let mut draft = static_draft();
        draft.automatic_dns = true;
        draft.gateway = "192.0.2.1".into();
        assert!(draft.validate().is_err());
    }

    #[test]
    fn host_and_gateway_validation_respects_point_to_point_prefix() {
        let mut draft = static_draft();
        draft.address = "198.51.100.0".into();
        assert!(draft.validate().is_err());
        draft.subnet = "31".into();
        draft.gateway = "198.51.100.1".into();
        assert!(draft.validate().is_ok());
        draft.gateway = draft.address.clone();
        assert!(draft.validate().is_err());
    }

    #[test]
    fn empty_or_duplicate_manual_dns_is_not_accepted() {
        let mut draft = static_draft();
        draft.preferred_dns.clear();
        assert!(draft.validate().is_err());
        draft.preferred_dns = "192.0.2.53".into();
        draft.alternate_dns = draft.preferred_dns.clone();
        assert!(draft.validate().is_err());
    }

    #[test]
    fn local_resolver_can_use_loopback_without_allowing_loopback_adapter_addresses() {
        let mut draft = static_draft();
        draft.preferred_dns = "127.0.0.1".into();
        assert!(draft.validate().is_ok());
        draft.address = "127.0.0.2".into();
        assert!(draft.validate().is_err());
    }

    #[test]
    fn preview_keeps_target_identity_and_does_not_mutate_snapshot() {
        let adapter = sample_adapters().remove(0);
        let before = adapter.config.clone();
        let desired = static_draft().validate().unwrap();
        let plan = ChangePlan::preview(&adapter, &desired);
        assert_eq!(plan.target_id, Uuid::from_u128(1));
        assert!(plan.changes.iter().any(|change| change.field == "IP mode"));
        assert_eq!(adapter.config, before);
        assert!(ChangePlan::preview(&adapter, &before).changes.is_empty());
    }

    #[test]
    fn config_round_trips_through_draft() {
        let config = static_draft().validate().unwrap();
        assert_eq!(
            EditorDraft::from_config(&config).validate().unwrap(),
            config
        );
    }
}
