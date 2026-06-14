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

    /// Disable watching namespaces and namespace-level annotations (removes need for Namespace RBAC).
    #[arg(long, env = "DISABLE_NAMESPACE_REFLECTOR", default_value = "false")]
    pub disable_namespace_reflector: bool,

    /// HTTP/2 keep-alive interval in seconds. If set to 0, HTTP/2 keep-alives are disabled.
    #[arg(long, env = "HTTP2_KEEP_ALIVE_INTERVAL_SECS", default_value = "0")]
    pub http2_keep_alive_interval_secs: u64,

    /// HTTP/2 keep-alive timeout in seconds.
    #[arg(long, env = "HTTP2_KEEP_ALIVE_TIMEOUT_SECS", default_value = "20")]
    pub http2_keep_alive_timeout_secs: u64,

    /// HTTP/2 max concurrent streams. If set to 0, the default limit (200) is used.
    #[arg(long, env = "HTTP2_MAX_CONCURRENT_STREAMS", default_value = "0")]
    pub http2_max_concurrent_streams: u32,

    /// Disable the Prometheus metrics endpoint.
    #[arg(long, env = "DISABLE_METRICS", default_value = "false")]
    pub disable_metrics: bool,

    /// Port to expose Prometheus metrics on.
    #[arg(long, env = "METRICS_PORT", default_value = "9090")]
    pub metrics_port: u16,

    /// Bind address for Prometheus metrics.
    #[arg(long, env = "METRICS_BIND_ADDRESS", default_value = "0.0.0.0")]
    pub metrics_bind_address: String,
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
            disable_namespace_reflector: false,
            http2_keep_alive_interval_secs: 0,
            http2_keep_alive_timeout_secs: 20,
            http2_max_concurrent_streams: 0,
            disable_metrics: false,
            metrics_port: 9090,
            metrics_bind_address: "0.0.0.0".to_string(),
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
            disable_namespace_reflector: false,
            http2_keep_alive_interval_secs: 0,
            http2_keep_alive_timeout_secs: 20,
            http2_max_concurrent_streams: 0,
            disable_metrics: false,
            metrics_port: 9090,
            metrics_bind_address: "0.0.0.0".to_string(),
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
            disable_namespace_reflector: false,
            http2_keep_alive_interval_secs: 0,
            http2_keep_alive_timeout_secs: 20,
            http2_max_concurrent_streams: 0,
            disable_metrics: false,
            metrics_port: 9090,
            metrics_bind_address: "0.0.0.0".to_string(),
        };
        assert!(config.parse_default_sysctls().is_err());
    }
}
