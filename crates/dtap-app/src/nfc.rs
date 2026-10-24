/// Web NFC probe. The tap control is shown only when this check is true.
#[cfg(any(test, all(feature = "web", target_arch = "wasm32")))]
const NFC_PROBE_JS: &str = "dioxus.send(typeof window !== 'undefined' && 'NDEFReader' in window);";

/// Returns the first URL/text NDEF record from a tag (e.g. a Bolt Card's lnurlw://...).
#[cfg(feature = "web")]
pub async fn read_ndef_uri() -> Result<String, String> {
    let mut ev = dioxus::document::eval(
        r#"
        if (!('NDEFReader' in window)) {
            dioxus.send({ err: 'Web NFC unsupported (Chrome on Android only)' }); return;
        }
        const ctl = new AbortController();
        const reader = new NDEFReader();
        try {
            await reader.scan({ signal: ctl.signal });
            reader.onreading = (e) => {
                for (const rec of e.message.records) {
                    if (rec.recordType === 'url' || rec.recordType === 'text') {
                        const s = new TextDecoder(rec.encoding || 'utf-8').decode(rec.data);
                        ctl.abort(); dioxus.send({ ok: s }); return;
                    }
                }
            };
        } catch (err) { dioxus.send({ err: String(err) }); }
    "#,
    );
    let v: serde_json::Value = ev.recv().await.map_err(|e| format!("{e:?}"))?;
    match v["ok"].as_str() {
        Some(s) => Ok(s.to_string()),
        None => Err(v["err"].as_str().unwrap_or("NFC read failed").to_string()),
    }
}

#[cfg(not(feature = "web"))]
pub async fn read_ndef_uri() -> Result<String, String> {
    Err("Native NFC bridge not wired yet. Use QR".into())
}

/// Web builds ask the browser. Desktop and other hosts do not pretend Web NFC exists.
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub fn use_nfc_available() -> bool {
    use dioxus::prelude::{spawn, use_effect, use_signal, WritableExt};
    let mut available = use_signal(|| false);
    use_effect(move || {
        spawn(async move {
            let mut ev = dioxus::document::eval(NFC_PROBE_JS);
            if let Ok(ok) = ev.recv::<bool>().await {
                available.set(ok);
            }
        });
    });
    available()
}

#[cfg(not(all(feature = "web", target_arch = "wasm32")))]
pub fn use_nfc_available() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_requires_web_nfc() {
        assert!(NFC_PROBE_JS.contains("'NDEFReader' in window"));
    }
}
