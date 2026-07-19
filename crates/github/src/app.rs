//! GitHub App client: app JWTs, installation tokens, and the handful of API
//! calls Projexity needs. Deliberately small — no octocrab, just reqwest.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

const API: &str = "https://api.github.com";
const UA: &str = "projexity";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .expect("reqwest client")
}

/// Short-lived (10 min max) JWT authenticating AS the app.
pub fn app_jwt(app_id: i64, private_key_pem: &str) -> anyhow::Result<String> {
    #[derive(Serialize)]
    struct Claims {
        iat: i64,
        exp: i64,
        iss: String,
    }
    let now = Utc::now().timestamp();
    let claims = Claims {
        // A minute of clock-drift allowance, per GitHub's docs.
        iat: now - 60,
        exp: now + 9 * 60,
        iss: app_id.to_string(),
    };
    let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())?;
    Ok(jsonwebtoken::encode(
        &Header::new(Algorithm::RS256),
        &claims,
        &key,
    )?)
}

/// Result of converting an app-manifest code: the app's full credentials.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestConversion {
    pub id: i64,
    pub slug: String,
    pub html_url: String,
    pub client_id: String,
    pub pem: String,
    pub webhook_secret: String,
}

/// Exchange the one-time code GitHub hands back after manifest creation for
/// the app's credentials. The code is valid for one hour, single use.
pub async fn convert_manifest_code(code: &str) -> anyhow::Result<ManifestConversion> {
    let res = client()
        .post(format!("{API}/app-manifests/{code}/conversions"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if !res.status().is_success() {
        anyhow::bail!(
            "GitHub rejected the app-manifest code (HTTP {}): {}",
            res.status(),
            res.text().await.unwrap_or_default()
        );
    }
    Ok(res.json().await?)
}

/// Mint a one-hour installation token.
pub async fn installation_token(
    app_id: i64,
    private_key_pem: &str,
    installation_id: i64,
) -> anyhow::Result<String> {
    let jwt = app_jwt(app_id, private_key_pem)?;
    #[derive(Deserialize)]
    struct TokenResponse {
        token: String,
    }
    let res = client()
        .post(format!(
            "{API}/app/installations/{installation_id}/access_tokens"
        ))
        .bearer_auth(jwt)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if !res.status().is_success() {
        anyhow::bail!(
            "installation token request failed (HTTP {}): {}",
            res.status(),
            res.text().await.unwrap_or_default()
        );
    }
    Ok(res.json::<TokenResponse>().await?.token)
}

/// The `http.extraheader` value that authenticates a git clone with an
/// installation token (never put tokens in clone URLs — they leak into error
/// messages).
pub fn clone_auth_header(installation_token: &str) -> String {
    let basic = B64.encode(format!("x-access-token:{installation_token}"));
    format!("Authorization: basic {basic}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoInfo {
    pub full_name: String,
    pub private: bool,
    pub default_branch: String,
}

/// Repositories this installation can reach (first 100 — pagination when
/// someone actually hits it).
pub async fn list_installation_repos(token: &str) -> anyhow::Result<Vec<RepoInfo>> {
    #[derive(Deserialize)]
    struct Page {
        repositories: Vec<RepoInfo>,
    }
    let res = client()
        .get(format!("{API}/installation/repositories?per_page=100"))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?;
    if !res.status().is_success() {
        anyhow::bail!("repo list failed (HTTP {})", res.status());
    }
    Ok(res.json::<Page>().await?.repositories)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_header_is_basic_auth() {
        let h = clone_auth_header("ghs_token123");
        assert!(h.starts_with("Authorization: basic "));
        let b64 = h.trim_start_matches("Authorization: basic ");
        let decoded = String::from_utf8(B64.decode(b64).unwrap()).unwrap();
        assert_eq!(decoded, "x-access-token:ghs_token123");
    }
}
