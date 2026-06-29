//! OAuth support for provider subscriptions (primarily OpenAI ChatGPT / Codex access).
//!
//! Allows users with ChatGPT Plus/Pro subscriptions to authenticate via OpenAI's
//! OAuth flow and use advanced models directly (no separate platform.openai.com API key required
//! for supported models).
//!
//! The flow uses PKCE + authorization code. We keep it CLI/TUI friendly by printing a URL
//! and asking the user to paste the redirect URL (or code) after browser login. This avoids
//! extra deps for a local HTTP server and works in most environments.

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};


const OPENAI_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const OPENAI_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const OPENAI_SCOPES: &str = "openid profile email offline_access";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OpenAIOAuthCreds {
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>, // unix timestamp seconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Generate a cryptographically suitable code verifier (43-128 chars, url-safe).
fn generate_code_verifier() -> String {
    let mut rng = rand::thread_rng();
    // 64 bytes -> ~86 chars after b64, but we use alphanum for simplicity and compatibility
    // OpenAI accepts 43-128 char verifiers.
    let bytes: Vec<u8> = (0..64).map(|_| rng.sample(Alphanumeric) as u8).collect();
    URL_SAFE_NO_PAD.encode(&bytes)[..64].to_string()
}

/// PKCE code_challenge = BASE64URL(SHA256(verifier)) (no padding)
fn code_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    URL_SAFE_NO_PAD.encode(digest)
}

/// Generate a random state for CSRF protection.
fn generate_state() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| rng.sample(Alphanumeric) as char)
        .collect()
}

/// Very small percent-encode for query values (sufficient for our params).
fn percent_encode(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                (b as char).to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect()
}

/// Build the full authorization URL the user should open in their browser.
pub fn build_openai_authorize_url(verifier: &str, state: &str) -> String {
    let challenge = code_challenge(verifier);
    let params: Vec<(&str, String)> = vec![
        ("response_type", "code".to_string()),
        ("client_id", OPENAI_CLIENT_ID.to_string()),
        ("redirect_uri", percent_encode(OPENAI_REDIRECT_URI)),
        ("scope", percent_encode(OPENAI_SCOPES)),
        ("code_challenge", percent_encode(&challenge)),
        ("code_challenge_method", "S256".to_string()),
        ("state", percent_encode(state)),
    ];

    let query = params
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&");

    format!("{}?{}", OPENAI_AUTHORIZE_URL, query)
}

/// Parse a pasted redirect URL or raw code.
/// Accepts either the full http://localhost... ?code=XXX&state=YYY
/// or just the code value.
pub fn parse_auth_code(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // If it looks like a URL containing code=, extract it
    if trimmed.contains("code=") {
        for part in trimmed.split(|c| c == '?' || c == '&' || c == '#') {
            if let Some(code) = part.strip_prefix("code=") {
                if !code.is_empty() {
                    // stop at first & if any
                    return Some(code.split('&').next().unwrap_or(code).to_string());
                }
            }
        }
    }

    // Otherwise treat input (trimmed) as the raw code
    Some(trimmed.to_string())
}

/// Save full OAuth credentials (including refresh token) into the keyring for the provider.
/// The JSON blob is stored; get_credential will extract the access_token.
pub fn save_oauth_creds(provider_name: &str, creds: &OpenAIOAuthCreds) -> Result<()> {
    let entry_name = format!("provider:{}", provider_name);
    let entry = keyring::Entry::new("anvil", &entry_name)
        .map_err(|e| anyhow!("keyring entry error for {}: {}", provider_name, e))?;
    let json = serde_json::to_string(creds)
        .map_err(|e| anyhow!("failed to serialize oauth creds: {}", e))?;
    entry
        .set_password(&json)
        .map_err(|e| anyhow!("failed to store oauth creds for {}: {}", provider_name, e))?;
    Ok(())
}

/// Load OAuth creds if the keyring entry for the provider contains a JSON oauth blob.
/// Returns None for plain API key entries (standard usage).
pub fn load_oauth_creds(provider_name: &str) -> Result<Option<OpenAIOAuthCreds>> {
    let entry_name = format!("provider:{}", provider_name);
    let entry = keyring::Entry::new("anvil", &entry_name)
        .map_err(|e| anyhow!("keyring entry error for {}: {}", provider_name, e))?;
    match entry.get_password() {
        Ok(s) => {
            let trimmed = s.trim();
            if trimmed.starts_with('{') {
                if let Ok(c) = serde_json::from_str::<OpenAIOAuthCreds>(trimmed) {
                    if !c.access_token.is_empty() {
                        return Ok(Some(c));
                    }
                }
            }
            Ok(None)
        }
        Err(_) => Ok(None),
    }
}

/// Exchange the authorization code for tokens.
async fn exchange_code_for_tokens(code: &str, verifier: &str) -> Result<OpenAIOAuthCreds> {
    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "authorization_code"),
        ("client_id", OPENAI_CLIENT_ID),
        ("code", code),
        ("code_verifier", verifier),
        ("redirect_uri", OPENAI_REDIRECT_URI),
    ];

    let resp = client
        .post(OPENAI_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await
        .context("failed to reach OpenAI token endpoint")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI token exchange failed ({}): {}", status, body);
    }

    #[derive(Deserialize)]
    struct TokenResp {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<i64>,
        // id_token may be present (not currently used)
        #[serde(default)]
        #[allow(dead_code)]
        id_token: Option<String>,
    }

    let token: TokenResp = resp.json().await.context("invalid token response json")?;

    let expires_at = token.expires_in.map(|secs| chrono::Utc::now().timestamp() + secs);

    // Try to extract account id from access_token JWT if possible (best effort)
    let account_id = extract_chatgpt_account_id(&token.access_token);

    Ok(OpenAIOAuthCreds {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_at,
        account_id,
        email: None, // can be populated from id_token if needed later
    })
}

/// Attempt to refresh using refresh_token.
pub fn refresh_openai_token(creds: &OpenAIOAuthCreds) -> Result<OpenAIOAuthCreds> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime for token refresh")
        .block_on(async { refresh_openai_token_async(creds).await })
}

async fn refresh_openai_token_async(creds: &OpenAIOAuthCreds) -> Result<OpenAIOAuthCreds> {
    let refresh = creds.refresh_token.as_ref().ok_or_else(|| anyhow!("no refresh token available"))?;

    let client = reqwest::Client::new();

    let params = [
        ("grant_type", "refresh_token"),
        ("client_id", OPENAI_CLIENT_ID),
        ("refresh_token", refresh),
    ];

    let resp = client
        .post(OPENAI_TOKEN_URL)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&params)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI token refresh failed ({}): {}", status, body);
    }

    #[derive(Deserialize)]
    struct RefreshResp {
        access_token: String,
        #[serde(default)]
        refresh_token: Option<String>,
        #[serde(default)]
        expires_in: Option<i64>,
    }

    let r: RefreshResp = resp.json().await?;

    let new_expires = r.expires_in.map(|s| chrono::Utc::now().timestamp() + s);

    // Preserve account id if we had one
    let account_id = creds.account_id.clone().or_else(|| extract_chatgpt_account_id(&r.access_token));

    Ok(OpenAIOAuthCreds {
        access_token: r.access_token,
        refresh_token: r.refresh_token.or_else(|| creds.refresh_token.clone()),
        expires_at: new_expires,
        account_id,
        email: creds.email.clone(),
    })
}

/// Best-effort extraction of chatgpt_account_id from the JWT access token (in the private claim).
fn extract_chatgpt_account_id(access_token: &str) -> Option<String> {
    // JWT = header.payload.signature
    let parts: Vec<&str> = access_token.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let payload_b64 = parts[1];
    // Add padding if needed
    let mut padded = payload_b64.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let decoded = URL_SAFE_NO_PAD.decode(payload_b64).or_else(|_| base64::engine::general_purpose::STANDARD.decode(&padded)).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;

    // The claim used by OpenAI Codex / ChatGPT OAuth
    if let Some(auth) = json.get("https://api.openai.com/auth") {
        if let Some(id) = auth.get("chatgpt_account_id").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    // Some tokens put it at top level
    if let Some(id) = json.get("chatgpt_account_id").and_then(|v| v.as_str()) {
        if !id.is_empty() {
            return Some(id.to_string());
        }
    }
    None
}

/// Perform the full interactive login flow for OpenAI ChatGPT subscription.
/// Returns the credentials (caller is responsible for persisting them).
///
/// The caller should:
///   1. Print the URL or let the function handle instructions.
///   2. Use inquire (or other) to ask user to paste the result after browser auth.
///   3. Store the resulting OpenAIOAuthCreds securely (keyring) + account_id in provider config.
pub fn login_openai_subscription() -> Result<OpenAIOAuthCreds> {
    // Run the async body in a current-thread runtime (same pattern Anvil uses elsewhere).
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build runtime for OAuth login")
        .block_on(async { login_openai_subscription_async().await })
}

async fn login_openai_subscription_async() -> Result<OpenAIOAuthCreds> {
    let verifier = generate_code_verifier();
    let state = generate_state();

    let url = build_openai_authorize_url(&verifier, &state);

    println!("\nOpenAI ChatGPT subscription login");
    println!("1. Open this URL in your browser and log in with your ChatGPT / OpenAI account:");
    println!("   {}\n", url);
    println!("2. After successful login you will be redirected (may show localhost page).");
    println!("3. Copy the FULL redirect URL from the address bar (or just the 'code' value) and paste it here.\n");

    let input: String = inquire::Text::new("Paste the redirect URL or code:")
        .prompt()?;

    let code = parse_auth_code(&input)
        .ok_or_else(|| anyhow!("could not extract authorization code from input"))?;

    let creds = exchange_code_for_tokens(&code, &verifier).await?;

    if creds.account_id.is_none() {
        // Still usable, account id can sometimes be derived on first call
    }

    Ok(creds)
}

/// Check if the creds are expired (with small buffer).
pub fn is_oauth_expired(creds: &OpenAIOAuthCreds) -> bool {
    if let Some(exp) = creds.expires_at {
        let now = chrono::Utc::now().timestamp();
        now > exp - 60 // 1 minute buffer
    } else {
        false
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_code_from_url() {
        let url = "http://localhost:1455/auth/callback?code=abc123&state=xyz";
        assert_eq!(parse_auth_code(url), Some("abc123".to_string()));
    }

    #[test]
    fn test_parse_raw_code() {
        assert_eq!(parse_auth_code("  justthecode  "), Some("justthecode".to_string()));
    }
}