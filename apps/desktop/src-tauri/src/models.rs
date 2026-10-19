use serde::Serialize;
use ts_rs::TS;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T> {
    pub schema_version: u8,
    pub trace_id: String,
    pub timestamp: String,
    pub data: T,
}

#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ErrorDto {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub trace_id: String,
}

impl ErrorDto {
    pub fn new(code: &str, message: &str, retryable: bool) -> Self {
        Self { code: code.into(), message: message.into(), retryable, trace_id: uuid::Uuid::new_v4().to_string() }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResult<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope: Option<Envelope<T>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorDto>,
}

impl<T> CommandResult<T> {
    pub fn success(data: T) -> Self where T: Serialize {
        Self { ok: true, envelope: Some(Envelope { schema_version: 3, trace_id: uuid::Uuid::new_v4().to_string(), timestamp: chrono::Utc::now().to_rfc3339(), data }), error: None }
    }
    pub fn failure(error: ErrorDto) -> Self { Self { ok: false, envelope: None, error: Some(error) } }
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CapabilityDto {
    pub external_read: bool,
    pub external_select_proxy: bool,
    pub managed_core: bool,
    pub system_proxy: bool,
    pub tun: bool,
    pub diagnostics: bool,
    pub network_restore: bool,
    pub update: bool,
    pub reason_codes: Vec<String>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BootstrapDto {
    pub app_version: String,
    pub os_version: String,
    pub architecture: String,
    pub first_run: bool,
    pub core_mode: Option<String>,
    pub core_state: String,
    pub ingress: String,
    pub capabilities: CapabilityDto,
    pub settings_revision: u32,
}

impl BootstrapDto {
    pub fn new() -> Self {
        Self { app_version: env!("CARGO_PKG_VERSION").into(), os_version: std::env::consts::OS.into(), architecture: std::env::consts::ARCH.into(), first_run: true, core_mode: None, core_state: "DISCONNECTED".into(), ingress: "OFF".into(), capabilities: CapabilityDto { external_read: false, external_select_proxy: false, managed_core: false, system_proxy: false, tun: false, diagnostics: false, network_restore: false, update: false, reason_codes: vec!["S0_NOT_VERIFIED".into()] }, settings_revision: 0 }
    }
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EnvironmentDto {
    pub os_version: String,
    pub architecture: String,
    pub is_macos: bool,
    pub observed_at: String,
    pub network_status: String,
    pub proxy_state: String,
    pub proxy_reason_code: String,
}

impl EnvironmentDto {
    pub fn inspect() -> Self {
        let proxy = crate::environment::observe_proxy();
        Self { os_version: std::env::consts::OS.into(), architecture: std::env::consts::ARCH.into(), is_macos: cfg!(target_os = "macos"), observed_at: chrono::Utc::now().to_rfc3339(), network_status: "UNKNOWN".into(),
            proxy_state: proxy.state.into(), proxy_reason_code: proxy.reason_code.into() }
    }
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoreSnapshotDto {
    pub state: String,
    pub observed_at: String,
    pub core_version: String,
    #[ts(type = "number")]
    pub connection_count: u64,
    #[ts(type = "number | null")]
    pub upload_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub download_bytes: Option<u64>,
    pub route_evidence: Vec<RouteEvidenceDto>,
    #[ts(type = "number")]
    pub unknown_route_count: u64,
    #[ts(type = "number")]
    pub identified_process_count: u64,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RouteEvidenceDto {
    pub chain: String,
    #[ts(type = "number")]
    pub connection_count: u64,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoreSessionDto {
    pub mode: String,
    pub state: String,
    pub version: String,
    pub endpoint: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionDto {
    pub id: String,
    pub host: Option<String>,
    pub destination_ip: Option<String>,
    pub process_name: Option<String>,
    pub rule: Option<String>,
    pub chains: Vec<String>,
    #[ts(type = "number | null")]
    pub upload_bytes: Option<u64>,
    #[ts(type = "number | null")]
    pub download_bytes: Option<u64>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ConnectionPageDto {
    pub items: Vec<ConnectionDto>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
    pub total: usize,
    pub skipped_count: usize,
    pub observed_at: String,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiagnosticProbeDto {
    pub name: String,
    pub status: String,
    #[ts(type = "number")]
    pub latency_ms: u64,
    pub reason_code: Option<String>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiagnosticReportDto {
    pub observed_at: String,
    pub scope: String,
    pub probes: Vec<DiagnosticProbeDto>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_never_claims_network_ownership() {
        let b = BootstrapDto::new();
        assert_eq!(b.core_state, "DISCONNECTED");
        assert_eq!(b.ingress, "OFF");
        assert!(!b.capabilities.tun);
    }
    #[test]
    fn envelope_has_contract_version() {
        let v = serde_json::to_value(CommandResult::success(BootstrapDto::new())).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["envelope"]["schemaVersion"], 3);
    }
}
