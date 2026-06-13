use clap::Parser;
use std::collections::HashMap;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Port to listen on.
    #[arg(short, long, env = "PORT", default_value = "8443")]
    pub port: u16,

    /// Bind address.
    #[arg(short, long, env = "BIND_ADDRESS", default_value = "0.0.0.0")]
    pub bind_address: String,

    /// Path to TLS certificate.
    #[arg(long, env = "TLS_CERT", default_value = "/etc/webhook/certs/tls.crt")]
    pub tls_cert: String,

    /// Path to TLS private key.
    #[arg(long, env = "TLS_KEY", default_value = "/etc/webhook/certs/tls.key")]
    pub tls_key: String,

    /// Default fallback sysctls as a JSON object (e.g. `'{"net.ipv4.ip_local_port_range": "1024 65000"}'`).
    #[arg(long, env = "DEFAULT_SYSCTLS", default_value = "{}")]
    pub default_sysctls: String,
}

impl Config {
    /// Parse the default sysctls JSON string into a `HashMap`.
    pub fn parse_default_sysctls(&self) -> Result<HashMap<String, String>, serde_json::Error> {
        serde_json::from_str(&self.default_sysctls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_sysctls_empty() {
        let config = Config {
            port: 8443,
            bind_address: "0.0.0.0".to_string(),
            tls_cert: String::new(),
            tls_key: String::new(),
            default_sysctls: "{}".to_string(),
        };
        let sysctls = config.parse_default_sysctls().unwrap();
        assert!(sysctls.is_empty());
    }

    #[test]
    fn test_parse_default_sysctls_valid() {
        let config = Config {
            port: 8443,
            bind_address: "0.0.0.0".to_string(),
            tls_cert: String::new(),
            tls_key: String::new(),
            default_sysctls: r#"{"net.ipv4.ip_local_port_range": "1024 65000"}"#.to_string(),
        };
        let sysctls = config.parse_default_sysctls().unwrap();
        assert_eq!(sysctls.len(), 1);
        assert_eq!(
            sysctls.get("net.ipv4.ip_local_port_range").unwrap(),
            "1024 65000"
        );
    }

    #[test]
    fn test_parse_default_sysctls_invalid() {
        let config = Config {
            port: 8443,
            bind_address: "0.0.0.0".to_string(),
            tls_cert: String::new(),
            tls_key: String::new(),
            default_sysctls: "invalid-json".to_string(),
        };
        assert!(config.parse_default_sysctls().is_err());
    }
}
