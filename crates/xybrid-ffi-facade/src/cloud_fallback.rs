//! Shared cloud-fallback metadata lowering for every FFI binding.

use crate::{Error, Result, RunOptions};
use url::Url;

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

impl RunOptions {
    /// Apply caller-resolved cloud overrides to an SDK envelope.
    ///
    /// Omitted or blank fields leave existing metadata untouched. Explicit
    /// fields override envelope metadata; no provider, model or gateway default
    /// is chosen here. Disabling fallback makes this a no-op, including URL
    /// validation. This configures the existing cloud leg, not a routing policy
    /// or an opt-in to speculative cloud execution.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ConfigError`] for an invalid active gateway override,
    /// before modifying any metadata. Gateway restrictions match Flutter's
    /// existing policy: HTTPS Xybrid hosts in release, plus local development
    /// gateways in debug builds, with a `/v1` base and no credentials/query/fragment.
    pub fn apply_cloud_fallback_metadata(
        &self,
        envelope: &mut xybrid_sdk::ir::Envelope,
    ) -> Result<()> {
        if !self.fallback_to_cloud {
            return Ok(());
        }
        let gateway = self.validated_cloud_gateway_url()?;
        let provider = non_empty(self.cloud_provider.as_deref());
        let model = non_empty(self.cloud_model.as_deref());
        if provider.is_none() && model.is_none() && gateway.is_none() {
            return Ok(());
        }
        for (key, value) in [
            ("provider", provider),
            ("model", model),
            ("gateway_url", gateway.as_deref()),
        ] {
            if let Some(value) = value {
                envelope.metadata.insert(key.to_string(), value.to_string());
            }
        }
        envelope.metadata.insert("backend".into(), "gateway".into());
        // Preserve Flutter's explicit generation overrides on the cloud leg.
        // Never materialize model/global generation defaults into the envelope.
        if let Some(config) = &self.generation_config {
            if let Some(value) = config.max_tokens {
                envelope
                    .metadata
                    .insert("max_tokens".into(), value.to_string());
            }
            if let Some(value) = config.temperature {
                envelope
                    .metadata
                    .insert("temperature".into(), value.to_string());
            }
        }
        Ok(())
    }

    pub(crate) fn validated_cloud_gateway_url(&self) -> Result<Option<String>> {
        if !self.fallback_to_cloud {
            return Ok(None);
        }
        non_empty(self.cloud_gateway_url.as_deref())
            .map(validate_cloud_gateway_url)
            .transpose()
            .map_err(|message| Error::ConfigError { message })
    }
}

fn validate_cloud_gateway_url(gateway_url: &str) -> std::result::Result<String, String> {
    let parsed = Url::parse(gateway_url)
        .map_err(|e| format!("Invalid cloud gateway URL '{}': {}", gateway_url, e))?;
    match parsed.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Invalid cloud gateway URL '{}': unsupported scheme '{}'",
                gateway_url, scheme
            ));
        }
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("Invalid cloud gateway URL: credentials are not allowed".to_string());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(
            "Invalid cloud gateway URL: query strings and fragments are not allowed".to_string(),
        );
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "Invalid cloud gateway URL: host is required".to_string())?;
    if !is_v1_gateway_base(&parsed) {
        return Err("Invalid cloud gateway URL: base URL must include /v1".to_string());
    }

    if parsed.scheme() == "https" && is_xybrid_gateway_host(host) {
        return Ok(normalize_gateway_url(parsed));
    }

    #[cfg(debug_assertions)]
    {
        if is_debug_gateway_host(host) {
            return Ok(normalize_gateway_url(parsed));
        }
    }

    Err(
        "Invalid cloud gateway URL: release builds only allow HTTPS Xybrid gateway hosts"
            .to_string(),
    )
}

fn normalize_gateway_url(parsed: Url) -> String {
    parsed.as_str().trim_end_matches('/').to_string()
}

fn is_v1_gateway_base(parsed: &Url) -> bool {
    let path = parsed.path().trim_end_matches('/');
    path == "/v1" || path.starts_with("/v1/")
}

fn is_xybrid_gateway_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    host == "xybrid.dev" || host.ends_with(".xybrid.dev")
}

#[cfg(debug_assertions)]
fn is_debug_gateway_host(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }

    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        Ok(std::net::IpAddr::V6(ip)) => {
            ip.is_loopback() || is_ipv6_link_local(ip) || is_ipv6_unique_local(ip)
        }
        Err(_) => false,
    }
}

#[cfg(debug_assertions)]
fn is_ipv6_link_local(ip: std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

#[cfg(debug_assertions)]
fn is_ipv6_unique_local(ip: std::net::Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AbortSignal, CancellationToken, GenerationConfig};

    fn envelope() -> xybrid_sdk::ir::Envelope {
        let mut env = xybrid_sdk::ir::Envelope {
            kind: xybrid_sdk::ir::EnvelopeKind::Text("hello".into()),
            metadata: Default::default(),
        };
        for (key, value) in [
            ("provider", "existing-provider"),
            ("model", "existing-model"),
            ("gateway_url", "https://api.xybrid.dev/v1"),
            ("backend", "existing-backend"),
            ("custom", "keep"),
            ("max_tokens", "99"),
            ("temperature", "0.8"),
        ] {
            env.metadata.insert(key.into(), value.into());
        }
        env
    }

    #[test]
    fn omitted_overrides_leave_all_metadata_unchanged() {
        let options = RunOptions {
            fallback_to_cloud: true,
            generation_config: Some(GenerationConfig::greedy()),
            ..Default::default()
        };
        let mut env = envelope();
        let before = env.metadata.clone();
        options.apply_cloud_fallback_metadata(&mut env).unwrap();
        assert_eq!(env.metadata, before);
        let mut empty = xybrid_sdk::ir::Envelope {
            kind: xybrid_sdk::ir::EnvelopeKind::Text("hello".into()),
            metadata: Default::default(),
        };
        options.apply_cloud_fallback_metadata(&mut empty).unwrap();
        assert!(empty.metadata.is_empty());
    }

    #[test]
    fn blank_overrides_are_omitted() {
        let options = RunOptions {
            fallback_to_cloud: true,
            cloud_provider: Some(" ".into()),
            cloud_model: Some("\t".into()),
            cloud_gateway_url: Some("\n".into()),
            ..Default::default()
        };
        let mut env = envelope();
        let before = env.metadata.clone();
        options.apply_cloud_fallback_metadata(&mut env).unwrap();
        assert_eq!(env.metadata, before);
    }

    #[test]
    fn explicit_fields_override_metadata_and_preserve_zero_sampling_values() {
        let options = RunOptions {
            fallback_to_cloud: true,
            cloud_provider: Some(" provider-x ".into()),
            cloud_model: Some(" model-y ".into()),
            cloud_gateway_url: Some(" https://api.xybrid.dev/v1/ ".into()),
            generation_config: Some(GenerationConfig {
                max_tokens: Some(0),
                temperature: Some(0.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut env = envelope();
        options.apply_cloud_fallback_metadata(&mut env).unwrap();
        for (key, expected) in [
            ("provider", "provider-x"),
            ("model", "model-y"),
            ("gateway_url", "https://api.xybrid.dev/v1"),
            ("backend", "gateway"),
            ("max_tokens", "0"),
            ("temperature", "0"),
            ("custom", "keep"),
        ] {
            assert_eq!(env.metadata[key], expected, "{key}");
        }
    }

    #[test]
    fn partial_overrides_do_not_supply_other_fields() {
        for (key, options) in [
            (
                "provider",
                RunOptions {
                    cloud_provider: Some("override".into()),
                    ..Default::default()
                },
            ),
            (
                "model",
                RunOptions {
                    cloud_model: Some("override".into()),
                    ..Default::default()
                },
            ),
            (
                "gateway_url",
                RunOptions {
                    cloud_gateway_url: Some("https://gateway.xybrid.dev/v1".into()),
                    ..Default::default()
                },
            ),
        ] {
            let options = RunOptions {
                fallback_to_cloud: true,
                ..options
            };
            let mut env = xybrid_sdk::ir::Envelope {
                kind: xybrid_sdk::ir::EnvelopeKind::Text("hello".into()),
                metadata: Default::default(),
            };
            options.apply_cloud_fallback_metadata(&mut env).unwrap();
            assert_eq!(env.metadata.len(), 2);
            assert!(env.metadata.contains_key(key));
            assert_eq!(env.metadata["backend"], "gateway");
            let mut existing = envelope();
            let before = existing.metadata.clone();
            options
                .apply_cloud_fallback_metadata(&mut existing)
                .unwrap();
            for other in [
                "provider",
                "model",
                "gateway_url",
                "max_tokens",
                "temperature",
                "custom",
            ] {
                if other != key {
                    assert_eq!(existing.metadata[other], before[other]);
                }
            }
        }
    }

    #[test]
    fn disabled_fallback_is_authoritative_even_with_an_invalid_gateway() {
        let options = RunOptions {
            cloud_provider: Some("other".into()),
            cloud_model: Some("other".into()),
            cloud_gateway_url: Some("not a URL".into()),
            ..Default::default()
        };
        let mut env = envelope();
        let before = env.metadata.clone();
        options.apply_cloud_fallback_metadata(&mut env).unwrap();
        assert_eq!(env.metadata, before);
        assert!(!options.to_sdk(None).unwrap().abort_policy.fallback_to_cloud);
    }

    #[test]
    fn invalid_gateway_fails_atomically_and_as_a_config_error() {
        let options = RunOptions {
            fallback_to_cloud: true,
            cloud_provider: Some("other".into()),
            cloud_gateway_url: Some("https://api.xybrid.dev/v1?token=secret".into()),
            ..Default::default()
        };
        let mut env = envelope();
        let before = env.metadata.clone();
        let error = options.apply_cloud_fallback_metadata(&mut env).unwrap_err();
        assert!(matches!(error, Error::ConfigError { .. }));
        assert!(!error.to_string().contains("secret"));
        assert_eq!(env.metadata, before);
        assert!(matches!(
            options.to_sdk(None),
            Err(Error::ConfigError { .. })
        ));
    }

    #[test]
    fn cloud_overrides_preserve_policy_cancellation_and_model_defaults() {
        let cancel = CancellationToken::new();
        let options = RunOptions {
            fallback_to_cloud: true,
            abort_on: vec![AbortSignal::ThermalCritical],
            max_grace_tokens: 7,
            correlation_id: Some("trace".into()),
            cloud_model: Some("cloud-model".into()),
            generation_config: Some(GenerationConfig {
                temperature: Some(0.0),
                ..Default::default()
            }),
            ..Default::default()
        };
        let base = xybrid_sdk::GenerationConfig {
            max_tokens: 123,
            ..Default::default()
        };
        let sdk = options.to_sdk_over(Some(&cancel), base).unwrap();
        assert!(sdk.abort_policy.fallback_to_cloud);
        assert!(sdk
            .abort_policy
            .observes(xybrid_sdk::AbortSignal::ThermalCritical));
        assert!(sdk
            .abort_policy
            .observes(xybrid_sdk::AbortSignal::UserCancelled));
        assert_eq!(sdk.abort_policy.max_grace_tokens, 7);
        assert_eq!(sdk.correlation_id.as_deref(), Some("trace"));
        assert_eq!(sdk.generation_config.as_ref().unwrap().max_tokens, 123);
        assert_eq!(sdk.generation_config.as_ref().unwrap().temperature, 0.0);
        cancel.cancel();
        assert!(sdk.cancellation_token.unwrap().is_cancelled());
    }

    #[test]
    fn gateway_validation_keeps_flutter_restrictions() {
        assert_eq!(
            validate_cloud_gateway_url("https://api.xybrid.dev/v1/").unwrap(),
            "https://api.xybrid.dev/v1"
        );
        for invalid in [
            "not a URL",
            "ftp://api.xybrid.dev/v1",
            "http://api.xybrid.dev/v1",
            "https://example.com/v1",
            "https://xybrid.dev.example.com/v1",
            "https://user:password@api.xybrid.dev/v1",
            "https://api.xybrid.dev/v1?query=x",
            "https://api.xybrid.dev/v1#fragment",
            "https://api.xybrid.dev/",
            "https://api.xybrid.dev/v10",
        ] {
            assert!(validate_cloud_gateway_url(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn local_gateway_is_debug_only() {
        let result = validate_cloud_gateway_url("http://127.0.0.1:3001/v1/");
        if cfg!(debug_assertions) {
            assert_eq!(result.unwrap(), "http://127.0.0.1:3001/v1");
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn pipelines_reject_each_unsupported_cloud_override() {
        for options in [
            RunOptions {
                cloud_provider: Some("provider".into()),
                ..Default::default()
            },
            RunOptions {
                cloud_model: Some("model".into()),
                ..Default::default()
            },
            RunOptions {
                cloud_gateway_url: Some("https://api.xybrid.dev/v1".into()),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                crate::pipeline_run_options(options),
                Err(Error::ConfigError { .. })
            ));
        }
    }
}
