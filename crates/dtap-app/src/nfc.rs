/// Returns the first URL/text NDEF record from a tag (e.g. a Bolt Card's lnurlw://...).
#[cfg(feature = "web")]
pub async fn read_ndef_uri() -> Result<String, String> {
    let mut ev = dioxus::document::eval(r#"
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
    "#);
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

pub fn available() -> bool {
    cfg!(any(feature = "web", target_os = "android", target_os = "ios"))
}
