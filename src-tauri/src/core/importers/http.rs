//! Minimal blocking HTTP GET (ureq + rustls). Always call off the main thread.

use std::time::Duration;

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_BODY: u64 = 32 * 1024 * 1024;

pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

/// GET without treating 4xx/5xx as an error; transport failures are `Err`.
pub fn get(url: &str) -> Result<HttpResponse, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    let mut response = agent.get(url).call().map_err(|e| e.to_string())?;
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    Ok(HttpResponse { status, body })
}
