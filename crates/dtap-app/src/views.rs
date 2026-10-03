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
            if s.is_empty() { "0".into() } else { s }
        }
        "." => if cur.contains('.') { cur.into() } else { format!("{cur}.") },
        d => {
            if let Some((_, dec)) = cur.split_once('.') {
                if dec.len() >= 2 { return cur.into(); }
            }
            if cur == "0" { d.into() } else if cur.len() >= 9 { cur.into() } else { format!("{cur}{d}") }
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
        let req = CreateCharge { amount: amount(), currency: "USD".into(), memo: None };
        let res = api::create_charge(&req).await;
        busy.set(false);
        match res {
            Ok(c) => { nav.push(Route::Checkout { invoice_id: c.invoice_id }); }
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
#[component]
fn Qr(data: String) -> Element {
    let svg_str = QrCode::new(data.as_bytes())
        .map(|c| c.render::<svg::Color>()
            .min_dimensions(260, 260)
            .dark_color(svg::Color("#05010f"))
            .light_color(svg::Color("#e8fbff"))
            .build())
        .unwrap_or_default();
    rsx! { div { class: "qr", dangerous_inner_html: svg_str } }
}

#[component]
pub fn Checkout(invoice_id: String) -> Element {
    let mut selected = use_signal(|| 0usize);
    let mut status = use_signal(|| ChargeStatus::New);
    let mut tap_msg = use_signal(|| None::<String>);
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
                if let Ok(s) = api::get_status(&id).await {
                    status.set(s);
                    if s.is_final() { break; }
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
                api::tap_pay(&TapRequest { invoice_id: id, lnurlw: uri }).await
            }.await;
            tap_msg.set(Some(match res {
                Ok(_) => "TAP ACCEPTED // SETTLING".into(),
                Err(e) => format!("TAP FAILED // {e}"),
            }));
        }
    };

    let body = match &*charge.read() {
        None => rsx! { p { class: "dim blink", "// HANDSHAKE WITH BLACKVAULT…" } },
        Some(Err(e)) => rsx! { p { class: "err", "LINK ERROR: {e}" } },
        Some(Ok(c)) => {
            let sel = selected();
            let chips: Vec<(usize, &'static str, String)> = c.rails.iter().enumerate()
                .map(|(i, r)| (i, r.label(), format!("rail {} {}", r.css(), if i == sel { "active" } else { "" })))
                .collect();
            let rail = c.rails.get(sel).cloned();
            let has_ln = c.rails.iter().any(|r| r.is_lightning());
            let total = format!("{} {}", c.amount, c.currency);
            let link = c.checkout_link.clone();
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
                    p { class: "mono", "DUE {r.amount_due} · RATE {r.rate}" }
                    p { class: "mono", "{r.destination}" }
                }
                if has_ln && nfc::available() {
                    button { class: "btn tap", onclick: tap, "◉ TAP BOLT CARD" }
                }
                if let Some(m) = tap_msg() { p { class: "mono", "{m}" } }
                a { class: "btn ghost", href: "{link}", target: "_blank", "OPEN FULL CHECKOUT" }
            }
        }
    };

    let st = status();
    let st_label = format!("{st:?}").to_uppercase();
    let st_class = format!("status {}", st.css());

    rsx! {
        section { class: "panel",
            div { class: "{st_class}", "// {st_label}" }
            {body}
        }
        if st == ChargeStatus::Settled {
            div { class: "paid-overlay",
                h2 { class: "glitch", "data-text": "PAID", "PAID" }
                button { class: "btn", onclick: move |_| { nav.push(Route::Terminal {}); }, "NEW SALE" }
            }
        }
    }
}

// ───────────── EXCHANGE LAB ─────────────
const COINS: &[(&str, &str)] = &[
    ("btc", "BTC"), ("xmr", "XMR"), ("usdt-tron", "USDT·TRC20"), ("usdc-ethereum", "USDC·ERC20"),
];

fn pill(active: bool) -> &'static str { if active { "rail alt active" } else { "rail alt" } }

#[component]
pub fn Swap() -> Element {
    let mut from = use_signal(|| "btc".to_string());
    let mut to = use_signal(|| "xmr".to_string());
    let quote = use_resource(move || {
        let (f, t) = (from(), to());
        async move {
            if f == t { return Err("PICK TWO DIFFERENT RAILS".to_string()); }
            api::quote(&f, &t).await
        }
    });

    let view = match &*quote.read() {
        None => rsx! { p { class: "dim blink", "// PINGING SWAP RAIL…" } },
        Some(Err(e)) => rsx! { p { class: "err", "{e}" } },
        Some(Ok(q)) => {
            let line = format!("1 {} → {} {}", q.from.to_uppercase(), q.rate, q.to.to_uppercase());
            let limits = format!("MIN {} · MAX {}", q.min, q.max);
            rsx! { div { class: "quote", "{line}" } p { class: "mono", "{limits}" } }
        }
    };

    rsx! {
        section { class: "panel",
            p { class: "label", "SEND" }
            div { class: "rails",
                for (id, name) in COINS.iter().copied() {
                    button { key: "f-{id}", class: pill(from() == id), onclick: move |_| from.set(id.to_string()), "{name}" }
                }
            }
            p { class: "label", "RECEIVE" }
            div { class: "rails",
                for (id, name) in COINS.iter().copied() {
                    button { key: "t-{id}", class: pill(to() == id), onclick: move |_| to.set(id.to_string()), "{name}" }
                }
            }
            {view}
            p { class: "dim pitch",
                "Customer pays in XMR, merchant settles in BTC or stablecoins. Same terminal, any rail."
            }
        }
    }
}
