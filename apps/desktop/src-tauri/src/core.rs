use crate::models::{CoreSnapshotDto, ErrorDto};
use serde_json::Value;
use std::time::Duration;
use url::{Host, Url};

#[derive(Clone)]
pub struct ExternalCore {
    pub endpoint: Url,
    pub version: String,
}

pub fn parse_loopback_endpoint(input: &str) -> Result<Url, ErrorDto> {
    let url = Url::parse(input).map_err(|_| ErrorDto::new("INVALID_ENDPOINT", "控制器地址格式错误", false))?;
    let loopback = match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if url.scheme() != "http" || !loopback
        || url.port().is_none() || url.username() != "" || url.password().is_some()
        || url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(ErrorDto::new("INVALID_ENDPOINT", "仅允许 http://127.0.0.1:端口 或 http://[::1]:端口", false));
    }
    Ok(url)
}

fn client() -> Result<reqwest::Client, ErrorDto> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| ErrorDto::new("CORE_CLIENT_ERROR", "无法创建本机控制器连接", false))
}

async fn get_json(endpoint: &Url, route: &str) -> Result<Value, ErrorDto> {
    let mut url = endpoint.clone();
    url.set_path(route);
    let response = client()?.get(url).send().await.map_err(|_| ErrorDto::new("CORE_UNAVAILABLE", "本机控制器未响应", true))?;
    match response.status().as_u16() {
        401 | 403 => return Err(ErrorDto::new("CORE_AUTH_REQUIRED", "控制器需要 Secret；当前版本暂不支持安全录入", false)),
        200..=299 => {},
        _ => return Err(ErrorDto::new("CORE_API_ERROR", "控制器返回异常状态", true)),
    }
    response.json::<Value>().await.map_err(|_| ErrorDto::new("CORE_RESPONSE_INVALID", "控制器数据格式错误", false))
}

pub async fn connect(endpoint_input: &str) -> Result<ExternalCore, ErrorDto> {
    let endpoint = parse_loopback_endpoint(endpoint_input)?;
    let version = get_json(&endpoint, "/version").await?;
    let version = version.get("version").and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "内核版本字段缺失", false))?;
    Ok(ExternalCore { endpoint, version: version.to_owned() })
}

pub async fn snapshot(core: &ExternalCore) -> Result<CoreSnapshotDto, ErrorDto> {
    let value = get_json(&core.endpoint, "/connections").await?;
    let connections = value.get("connections").and_then(Value::as_array)
        .ok_or_else(|| ErrorDto::new("CORE_RESPONSE_INVALID", "连接列表字段缺失", false))?;
    Ok(CoreSnapshotDto {
        state: "EXTERNAL_READONLY".into(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        core_version: core.version.clone(),
        connection_count: connections.len() as u64,
        upload_bytes: value.get("uploadTotal").and_then(Value::as_u64),
        download_bytes: value.get("downloadTotal").and_then(Value::as_u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_literal_loopback_only() {
        assert!(parse_loopback_endpoint("http://127.0.0.1:9090").is_ok());
        assert!(parse_loopback_endpoint("http://[::1]:9090").is_ok());
        for s in ["http://localhost:9090", "http://127.0.0.2:9090", "http://192.168.1.1:9090", "https://example.com", "http://127.0.0.1:9090/other", "http://127.0.0.1:9090?x=1", "http://u:p@127.0.0.1:9090"] {
            assert!(parse_loopback_endpoint(s).is_err(), "accepted {s}");
        }
    }
}
