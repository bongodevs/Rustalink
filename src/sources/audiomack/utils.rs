use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use sha1::Sha1;

const CONSUMER_KEY: &str = "audiomack-web";
const CONSUMER_SECRET: &str = "bd8a07e9f23fbe9d808646b730f89b8e";

type HmacSha1 = Hmac<Sha1>;

/// Percent encodes a string according to RFC 3986.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Builds an OAuth 1.0a Authorization header for Audiomack API requests.
pub fn build_auth_header(
    method: &str,
    url: &str,
    params: &BTreeMap<String, String>,
    nonce: &str,
    timestamp: &str,
) -> String {
    let mut oauth_params = BTreeMap::new();
    oauth_params.insert("oauth_consumer_key".to_owned(), CONSUMER_KEY.to_owned());
    oauth_params.insert("oauth_nonce".to_owned(), nonce.to_owned());
    oauth_params.insert("oauth_signature_method".to_owned(), "HMAC-SHA1".to_owned());
    oauth_params.insert("oauth_timestamp".to_owned(), timestamp.to_owned());
    oauth_params.insert("oauth_version".to_owned(), "1.0".to_owned());

    let mut all_params = oauth_params.clone();
    for (k, v) in params {
        all_params.insert(percent_encode(k), percent_encode(v));
    }

    let param_string = all_params
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");

    let base_string = format!(
        "{}&{}&{}",
        percent_encode(&method.to_uppercase()),
        percent_encode(url),
        percent_encode(&param_string)
    );

    let signing_key = format!("{}&", percent_encode(CONSUMER_SECRET));
    let mut mac =
        HmacSha1::new_from_slice(signing_key.as_bytes()).expect("HMAC can take any key size");
    mac.update(base_string.as_bytes());
    let signature = STANDARD.encode(mac.finalize().into_bytes());

    oauth_params.insert("oauth_signature".to_owned(), signature);

    let header_parts: Vec<_> = oauth_params
        .iter()
        .map(|(k, v)| format!("{}=\"{}\"", percent_encode(k), percent_encode(v)))
        .collect();

    format!("OAuth {}", header_parts.join(", "))
}
