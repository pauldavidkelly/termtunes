use color_eyre::Result;
use reqwest::header::HeaderMap;
use serde::Deserialize;

/// Response from the Plex PIN creation and check endpoints.
///
/// POST /api/v2/pins returns { id, code, authToken: null }
/// GET /api/v2/pins/{id} returns { id, code, authToken: "..." | null }
#[derive(Deserialize, Debug)]
pub struct PinResponse {
    pub id: u64,
    pub code: String,
    #[serde(rename = "authToken")]
    pub auth_token: Option<String>,
}

/// Build the common headers required on ALL plex.tv requests.
///
/// Includes X-Plex-Product ("TermTunes"), X-Plex-Client-Identifier (the
/// persistent UUID from config), and Accept: application/json (without this,
/// Plex returns XML).
pub fn build_plex_headers(client_id: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("X-Plex-Product", "TermTunes".parse().unwrap());
    headers.insert(
        "X-Plex-Client-Identifier",
        client_id.parse().unwrap(),
    );
    headers.insert("Accept", "application/json".parse().unwrap());
    headers
}

/// Create a new Plex authentication PIN.
///
/// POST https://plex.tv/api/v2/pins with query param strong=true.
/// Returns a PinResponse with a code the user can use to authenticate and
/// an id used to poll for completion.
pub async fn create_pin(
    client: &reqwest::Client,
    client_id: &str,
) -> Result<PinResponse> {
    let headers = build_plex_headers(client_id);
    let resp = client
        .post("https://plex.tv/api/v2/pins")
        .headers(headers)
        .query(&[("strong", "true")])
        .send()
        .await?
        .error_for_status()?
        .json::<PinResponse>()
        .await?;
    Ok(resp)
}

/// Check the status of a previously created PIN.
///
/// GET https://plex.tv/api/v2/pins/{pin_id}. The auth_token field will be
/// Some(token) once the user has authenticated via the browser.
pub async fn check_pin(
    client: &reqwest::Client,
    pin_id: u64,
    client_id: &str,
) -> Result<PinResponse> {
    let headers = build_plex_headers(client_id);
    let resp = client
        .get(format!("https://plex.tv/api/v2/pins/{}", pin_id))
        .headers(headers)
        .send()
        .await?
        .error_for_status()?
        .json::<PinResponse>()
        .await?;
    Ok(resp)
}

/// Start the Plex authentication flow by creating a PIN.
///
/// Returns `(pin_id, pin_code, auth_url)` so the caller can display the
/// URL to the user before polling. The auth URL is the Plex web page where
/// the user enters the PIN or clicks to authorize.
pub async fn start_auth(
    client: &reqwest::Client,
    client_id: &str,
) -> Result<(u64, String, String)> {
    let pin = create_pin(client, client_id).await?;
    let auth_url = format!(
        "https://app.plex.tv/auth#?clientID={}&code={}&context%5Bdevice%5D%5Bproduct%5D=TermTunes",
        client_id, pin.code
    );
    Ok((pin.id, pin.code, auth_url))
}

/// Poll for authentication completion after PIN creation.
///
/// Checks the PIN status every 1 second until auth_token is present.
/// Times out after 5 minutes (300 polls) and returns an error.
pub async fn wait_for_auth(
    client: &reqwest::Client,
    pin_id: u64,
    client_id: &str,
) -> Result<String> {
    for _ in 0..300 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let status = check_pin(client, pin_id, client_id).await?;
        if let Some(token) = status.auth_token {
            return Ok(token);
        }
    }
    Err(color_eyre::eyre::eyre!(
        "Authentication timed out after 5 minutes"
    ))
}

/// Validate an existing Plex auth token.
///
/// GET https://plex.tv/api/v2/user with X-Plex-Token header.
/// Returns true if the token is valid (200 response), false if unauthorized
/// (401), and propagates errors on network failure.
pub async fn validate_token(
    client: &reqwest::Client,
    token: &str,
    client_id: &str,
) -> Result<bool> {
    let mut headers = build_plex_headers(client_id);
    headers.insert("X-Plex-Token", token.parse().unwrap());
    let resp = client
        .get("https://plex.tv/api/v2/user")
        .headers(headers)
        .send()
        .await?;
    Ok(resp.status().is_success())
}
