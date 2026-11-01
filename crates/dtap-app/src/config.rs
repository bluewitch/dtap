//! Runtime terminal config: which gateway to call and with which terminal token.
//!
//! Nothing here is compiled in except the public dev defaults, which only work against
//! the local mock gateway (`make gateway`). A non-mock gateway refuses to start with the
//! dev token, so the default cannot unlock anything real.
//!
//! Web: the cashier pairs the device once on `/pair`; values live in this browser's
//! localStorage. Desktop/mobile: `DTAP_GATEWAY_URL` / `DTAP_TERMINAL_TOKEN` at runtime.

pub const DEV_GATEWAY: &str = "http://127.0.0.1:8787";
/// Public, dev-only value. Not a secret; accepted only by a gateway in mock mode.
pub const DEV_TOKEN: &str = "dev-terminal-token";

#[cfg(target_arch = "wasm32")]
const KEY_GATEWAY: &str = "dtap.gateway_url";
#[cfg(target_arch = "wasm32")]
const KEY_TOKEN: &str = "dtap.terminal_token";

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub gateway: String,
    pub token: String,
    pub paired: bool,
}

fn resolve(gateway: Option<String>, token: Option<String>) -> Config {
    let clean = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    match (clean(gateway), clean(token)) {
        (Some(g), Some(t)) => Config {
            gateway: g.trim_end_matches('/').to_string(),
            token: t,
            paired: true,
        },
        _ => Config {
            gateway: DEV_GATEWAY.into(),
            token: DEV_TOKEN.into(),
            paired: false,
        },
    }
}

/// Only https, except for a gateway on this machine during development.
pub fn validate_gateway(url: &str) -> Result<(), &'static str> {
    let u = url.trim();
    let local = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
        .iter()
        .any(|p| {
            u.starts_with(p) && matches!(u[p.len()..].chars().next(), None | Some(':') | Some('/'))
        });
    if u.starts_with("https://") && u.len() > "https://".len() || local {
        Ok(())
    } else {
        Err("Gateway must be https:// (http only for 127.0.0.1 / localhost)")
    }
}

pub fn validate_token(token: &str) -> Result<(), &'static str> {
    let t = token.trim();
    if t == DEV_TOKEN || t.len() < 32 {
        Err("Use the long random token from your gateway (32+ characters), not the dev token")
    } else {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
pub fn current() -> Config {
    let s = storage();
    let get = |k: &str| s.as_ref().and_then(|s| s.get_item(k).ok().flatten());
    resolve(get(KEY_GATEWAY), get(KEY_TOKEN))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn current() -> Config {
    resolve(
        std::env::var("DTAP_GATEWAY_URL").ok(),
        std::env::var("DTAP_TERMINAL_TOKEN").ok(),
    )
}

/// Save pairing on this device. Returns an error message for the cashier.
#[cfg(target_arch = "wasm32")]
pub fn save(gateway: &str, token: &str) -> Result<(), String> {
    validate_gateway(gateway)?;
    validate_token(token)?;
    let s = storage().ok_or("This browser has no local storage")?;
    s.set_item(KEY_GATEWAY, gateway.trim())
        .map_err(|_| "Could not save")?;
    s.set_item(KEY_TOKEN, token.trim())
        .map_err(|_| "Could not save")?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(gateway: &str, token: &str) -> Result<(), String> {
    validate_gateway(gateway)?;
    validate_token(token)?;
    Err("On desktop/mobile set DTAP_GATEWAY_URL and DTAP_TERMINAL_TOKEN in the environment".into())
}

#[cfg(target_arch = "wasm32")]
pub fn forget() {
    if let Some(s) = storage() {
        let _ = s.remove_item(KEY_GATEWAY);
        let _ = s.remove_item(KEY_TOKEN);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn forget() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpaired_falls_back_to_public_dev_defaults() {
        let c = resolve(None, Some("x".repeat(40)));
        assert_eq!(
            c,
            Config {
                gateway: DEV_GATEWAY.into(),
                token: DEV_TOKEN.into(),
                paired: false
            }
        );
        let c = resolve(Some("  ".into()), None);
        assert!(!c.paired);
    }

    #[test]
    fn paired_values_are_trimmed() {
        let c = resolve(Some(" https://gw.example/ ".into()), Some(" tok ".into()));
        assert_eq!(c.gateway, "https://gw.example");
        assert_eq!(c.token, "tok");
        assert!(c.paired);
    }

    #[test]
    fn gateway_must_be_https_unless_local() {
        assert!(validate_gateway("https://gw.example").is_ok());
        assert!(validate_gateway("http://127.0.0.1:8787").is_ok());
        assert!(validate_gateway("http://localhost").is_ok());
        assert!(validate_gateway("http://gw.example").is_err());
        assert!(validate_gateway("http://localhost.evil.example").is_err());
        assert!(validate_gateway("https://").is_err());
    }

    #[test]
    fn dev_or_short_tokens_are_refused_for_pairing() {
        assert!(validate_token(DEV_TOKEN).is_err());
        assert!(validate_token("short").is_err());
        assert!(validate_token(&"a".repeat(32)).is_ok());
    }
}
