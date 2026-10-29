use std::fs;

fn extract_json_str<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("\"{key}\"");
    let start = text.find(&pattern)? + pattern.len();
    let rest = &text[start..];
    let colon = rest.find(':')? + 1;
    let after_colon = rest[colon..].trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let content = &after_colon[1..];
    let end = content.find('"')?;
    Some(content[..end].trim())
}

fn main() {
    println!("cargo:rerun-if-changed=oauth-client.json");
    println!("cargo:rerun-if-changed=updater-config.json");
    if let Ok(raw) = fs::read_to_string("oauth-client.json")
        && let Some(id) = extract_json_str(&raw, "client_id")
        && id.ends_with(".apps.googleusercontent.com")
    {
        println!("cargo:rustc-env=NIKI_GOOGLE_CLIENT_ID={id}");
        if let Some(secret) = extract_json_str(&raw, "client_secret")
            && !secret.is_empty()
        {
            println!("cargo:rustc-env=NIKI_GOOGLE_CLIENT_SECRET={secret}");
        }
    }
    if let Ok(raw) = fs::read_to_string("updater-config.json") {
        if let Some(endpoint) = extract_json_str(&raw, "endpoint")
            && !endpoint.is_empty()
        {
            println!("cargo:rustc-env=NIKI_UPDATE_ENDPOINT={endpoint}");
        }
        if let Some(pubkey) = extract_json_str(&raw, "pubkey")
            && !pubkey.is_empty()
        {
            println!("cargo:rustc-env=NIKI_UPDATE_PUBKEY={pubkey}");
        }
    }
    tauri_build::build();
}
