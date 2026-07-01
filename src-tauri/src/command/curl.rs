//! Curl HTTP request command

#[derive(Debug, serde::Serialize)]
pub struct CurlResponse {
    pub status_code: u16,
    pub status_text: String,
    // ponytail: `Vec` (not `HashMap`) preserves duplicate header names, e.g. multiple
    // `set-cookie`. The frontend renders them as multi-value rows / a Cookie block.
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub duration_ms: i64,
}

#[tauri::command]
pub async fn send_curl_request(
    method: String,
    url: String,
    headers: std::collections::HashMap<String, String>,
    body: String,
    follow_redirects: bool,
    timeout: u64,
) -> Result<CurlResponse, String> {
    let start = std::time::Instant::now();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout.max(1).min(120)))
        .redirect(if follow_redirects {
            reqwest::redirect::Policy::limited(10)
        } else {
            reqwest::redirect::Policy::none()
        })
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let method = reqwest::Method::from_bytes(method.as_bytes())
        .map_err(|e| format!("Invalid HTTP method: {}", e))?;

    let mut request_builder = client.request(method, &url);
    for (key, value) in &headers {
        request_builder = request_builder.header(key, value);
    }
    if !body.is_empty() {
        request_builder = request_builder.body(body.clone());
    }

    let resp = request_builder
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = resp.status();
    let duration = start.elapsed().as_millis() as i64;

    let resp_headers = resp
        .headers()
        .iter()
        .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
        .collect();

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    Ok(CurlResponse {
        status_code: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("").to_string(),
        headers: resp_headers,
        body,
        duration_ms: duration,
    })
}
