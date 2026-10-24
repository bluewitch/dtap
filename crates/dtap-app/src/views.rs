use crate::{api, nfc, Route};
use dioxus::prelude::*;
use dtap_core::*;
use qrcode::{render::svg, QrCode};
use std::time::Duration;

// ───────────── TERMINAL ─────────────
const KEYS: [&str; 12] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", ".", "0", "⌫"];

fn apply_key(cur: &str, k: &str) -> String {
    match k {
        "⌫" => {
            let mut s = cur.to_string();
            s.pop();
            if s.is_empty() {
                "0".into()
            } else {
                s
            }
        }
        "." => {
            if cur.contains('.') {
                cur.into()
            } else {
                format!("{cur}.")
            }
        }
        d => {
            if let Some((_, dec)) = cur.split_once('.') {
                if dec.len() >= 2 {
                    return cur.into();
                }
            }
            if cur == "0" {
                d.into()
            } else if cur.len() >= 9 {
                cur.into()
            } else {
                format!("{cur}{d}")
            }
        }
    }
}

#[component]
pub fn Terminal() -> Element {
    let mut amount = use_signal(|| "0".to_string());
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let nav = navigator();

    let charge = move |_: Event<MouseData>| async move {
        busy.set(true);
        error.set(None);
        let req = CreateCharge {
            amount: amount(),
            currency: "USD".into(),
            memo: None,
        };
        let res = api::create_charge(&req).await;
        busy.set(false);
        match res {
            Ok(c) => {
                nav.push(Route::Checkout {
                    invoice_id: c.invoice_id,
                });
            }
            Err(e) => error.set(Some(e)),
        }
    };

    rsx! {
        section { class: "panel",
            p { class: "label", "CHARGE // USD" }
            div { class: "amount", "${amount}" }
            div { class: "keypad",
                for k in KEYS {
                    button {
                        key: "{k}",
                        class: "key",
                        onclick: move |_| {
                            let next = apply_key(&amount.read(), k);
                            amount.set(next);
                        },
                        "{k}"
                    }
                }
            }
            if let Some(e) = error() { p { class: "err", "{e}" } }
            button {
                class: "btn",
                disabled: busy() || amount() == "0",
                onclick: charge,
                if busy() { "ROUTING…" } else { "⟶ CHARGE" }
            }
        }
    }
}

// ───────────── CHECKOUT ─────────────
fn settleable(rail: &Rail) -> bool {
    matches!(rail.css(), "ln" | "btc")
}

fn usd_cents(amount: &str) -> u64 {
    let trimmed = amount.trim().trim_end_matches('.');
    let (whole, frac) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    if whole.chars().any(|c| !c.is_ascii_digit()) || frac.chars().any(|c| !c.is_ascii_digit()) {
        return 0;
    }
    let dollars: u64 = if whole.is_empty() {
        0
    } else {
        whole.parse().unwrap_or(0)
    };
    let mut frac_digits: String = frac.chars().take(2).collect();
    while frac_digits.len() < 2 {
        frac_digits.push('0');
    }
    let cents_part: u64 = frac_digits.parse().unwrap_or(0);
    dollars.saturating_mul(100).saturating_add(cents_part)
}

fn format_usd(amount: &str) -> String {
    let cents = usd_cents(amount);
    format!("${}.{:02} USD", cents / 100, cents % 100)
}

fn short_invoice(id: &str) -> String {
    let hex: String = id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(8)
        .collect();
    if hex.len() >= 4 {
        hex.to_ascii_uppercase()
    } else {
        id.chars().take(8).collect::<String>().to_ascii_uppercase()
    }
}

fn show_checkout_link(link: &str) -> bool {
    let link = link.trim();
    !link.is_empty() && !link.to_ascii_lowercase().contains("demo.dtap.local")
}

fn demo_payload(rail: &Rail) -> bool {
    let link = rail.payment_link.as_deref().unwrap_or("");
    rail.destination.to_ascii_lowercase().contains("demo")
        || link.to_ascii_lowercase().contains("demo")
}

/// Tap failures stay cashier-readable and always end by pointing at the QR.
fn tap_failure(err: &str) -> String {
    let e = err.to_ascii_lowercase();
    let msg = if e.contains("not implemented")
        || e.contains("501")
        || e.contains("not configured")
        || e.contains("not wired")
    {
        "BOLT CARD TAP IS NOT AVAILABLE, use QR"
    } else if e.contains("unsupported") || e.contains("nfc") || e.contains("ndef") {
        "WEB NFC IS NOT AVAILABLE ON THIS DEVICE, use QR"
    } else {
        "TAP FAILED, use QR"
    };
    msg.to_string()
}

fn stop_polling(err: &str) -> bool {
    err.starts_with("404")
}

#[component]
fn Qr(data: String) -> Element {
    let svg_str = QrCode::new(data.as_bytes())
        .map(|c| {
            c.render::<svg::Color>()
                .min_dimensions(260, 260)
                .dark_color(svg::Color("#05010f"))
                .light_color(svg::Color("#e8fbff"))
                .build()
        })
        .unwrap_or_default();
    rsx! { div { class: "qr", dangerous_inner_html: svg_str } }
}

#[component]
pub fn Checkout(invoice_id: String) -> Element {
    let mut selected = use_signal(|| 0usize);
    let mut status = use_signal(|| ChargeStatus::New);
    let mut tap_msg = use_signal(|| None::<String>);
    let nfc_ok = nfc::use_nfc_available();
    let nav = navigator();

    let id = invoice_id.clone();
    let charge = use_resource(move || {
        let id = id.clone();
        async move { api::get_charge(&id).await }
    });

    let id = invoice_id.clone();
    use_future(move || {
        let id = id.clone();
        async move {
            loop {
                futures_timer::Delay::new(Duration::from_secs(2)).await;
                match api::get_status(&id).await {
                    Ok(s) => {
                        status.set(s);
                        if s.is_final() {
                            break;
                        }
                    }
                    Err(e) if stop_polling(&e) => break,
                    Err(_) => {}
                }
            }
        }
    });

    let id = invoice_id.clone();
    let tap = move |_: Event<MouseData>| {
        let id = id.clone();
        async move {
            tap_msg.set(Some("◉ HOLD CARD TO DEVICE…".into()));
            let res: Result<serde_json::Value, String> = async {
                let uri = nfc::read_ndef_uri().await?;
                api::tap_pay(&TapRequest {
                    invoice_id: id,
                    lnurlw: uri,
                })
                .await
            }
            .await;
            tap_msg.set(Some(match res {
                Ok(_) => "TAP ACCEPTED // SETTLING".into(),
                Err(e) => tap_failure(&e),
            }));
        }
    };

    let body = match &*charge.read() {
        None => rsx! { p { class: "dim blink", "// HANDSHAKE WITH BLACKVAULT…" } },
        Some(Err(e)) => rsx! { p { class: "err", "LINK ERROR: {e}" } },
        Some(Ok(c)) => {
            let rails: Vec<Rail> = c
                .rails
                .iter()
                .filter(|rail| settleable(rail))
                .cloned()
                .collect();
            let total = format!("{} {}", c.amount, c.currency);
            let link = c.checkout_link.clone();
            if rails.is_empty() {
                rsx! {
                    p { class: "label", "TOTAL" }
                    div { class: "amount small", "{total}" }
                    p { class: "err", "NO LIGHTNING OR ON-CHAIN RAIL" }
                }
            } else {
                let sel = selected().min(rails.len() - 1);
                let chips: Vec<(usize, &'static str, String)> = rails
                    .iter()
                    .enumerate()
                    .map(|(i, rail)| {
                        let class = format!(
                            "rail {} {}",
                            rail.css(),
                            if i == sel { "active" } else { "" }
                        );
                        (i, rail.label(), class)
                    })
                    .collect();
                let rail = rails.get(sel).cloned();
                let has_ln = rails
                    .iter()
                    .any(|rail| rail.is_lightning() || rail.css() == "ln");
                rsx! {
                    p { class: "label", "TOTAL" }
                    div { class: "amount small", "{total}" }
                    div { class: "rails",
                        for (i, label, cls) in chips {
                            button { key: "{i}", class: "{cls}", onclick: move |_| selected.set(i), "{label}" }
                        }
                    }
                    if let Some(r) = rail {
                        Qr { data: r.payment_link.clone().unwrap_or(r.destination.clone()) }
                        if demo_payload(&r) {
                            p { class: "mono demo-tag", "DEMO QR · NOT PAYABLE" }
                        }
                        p { class: "mono", "DUE {r.amount_due} · RATE {r.rate}" }
                        p { class: "mono", "{r.destination}" }
                    }
                    if has_ln && nfc_ok {
                        button { class: "btn tap", onclick: tap, "◉ TAP BOLT CARD" }
                    }
                    if let Some(m) = tap_msg() { p { class: "mono", "{m}" } }
                    if show_checkout_link(&link) {
                        a { class: "btn ghost", href: "{link}", target: "_blank", "OPEN FULL CHECKOUT" }
                    }
                }
            }
        }
    };

    let st = status();
    let st_label = format!("{st:?}").to_uppercase();
    let st_class = format!("status {}", st.css());
    let paid_amount = match &*charge.read() {
        Some(Ok(c)) => format_usd(&c.amount),
        _ => String::new(),
    };
    let paid_id = short_invoice(&invoice_id);

    rsx! {
        section { class: "panel",
            div { class: "{st_class}", "// {st_label}" }
            {body}
        }
        if st == ChargeStatus::Settled {
            div { class: "paid-overlay",
                h2 { class: "glitch", "data-text": "PAID", "PAID" }
                if !paid_amount.is_empty() {
                    p { class: "paid-amount", "{paid_amount}" }
                }
                p { class: "paid-id", "INVOICE {paid_id}" }
                button { class: "btn", onclick: move |_| { nav.push(Route::Terminal {}); }, "NEW SALE" }
            }
        }
    }
}

// ───────────── EXCHANGE LAB ─────────────
// Unlinked. The gateway cannot settle swaps, so the counter does not offer them.
#[component]
pub fn Swap() -> Element {
    rsx! {
        section { class: "panel",
            p { class: "label", "EXCHANGE" }
            p { class: "dim pitch",
                "Swap quotes are not wired. This terminal settles Lightning and on-chain Bitcoin only."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail(id: &str) -> Rail {
        Rail {
            id: id.into(),
            destination: String::new(),
            payment_link: None,
            amount_due: String::new(),
            rate: String::new(),
        }
    }

    #[test]
    fn only_lightning_and_onchain_are_shown() {
        assert!(settleable(&rail("BTC-LN")));
        assert!(settleable(&rail("BTC")));
        assert!(!settleable(&rail("XMR")));
        assert!(!settleable(&rail("LBTC")));
        assert!(!settleable(&rail("USDT")));
        assert!(!settleable(&rail("USDC")));
    }

    #[test]
    fn tap_failure_hides_the_raw_gateway_error() {
        let raw = r#"501 Not Implemented: {"error":"tap not configured"}"#;
        let msg = tap_failure(raw);
        assert!(msg.ends_with("use QR"));
        assert!(!msg.contains("501"));
        assert!(!msg.contains("Not Implemented"));
        assert!(!msg.contains("tap not configured"));
        assert!(tap_failure("Web NFC unsupported").ends_with("use QR"));
    }

    #[test]
    fn paid_copy_is_amount_and_short_invoice() {
        assert_eq!(format_usd("5"), "$5.00 USD");
        assert_eq!(format_usd("5.5"), "$5.50 USD");
        assert_eq!(
            short_invoice("550e8400-e29b-41d4-a716-446655440000"),
            "550E8400"
        );
    }

    #[test]
    fn fake_checkout_host_stays_hidden() {
        assert!(!show_checkout_link(""));
        assert!(!show_checkout_link("https://demo.dtap.local/i/abc"));
        assert!(show_checkout_link("https://pay.example/i/abc"));
    }

    #[test]
    fn polling_stops_on_not_found() {
        assert!(stop_polling(r#"404 Not Found: {"error":"not found"}"#));
        assert!(!stop_polling("error sending request"));
    }
}
