use dioxus::prelude::*;

mod api;
mod nfc;
mod views;
use views::{Checkout, Swap, Terminal};

const CSS: Asset = asset!("/assets/cyber.css");
const FONTS: &str = "https://fonts.googleapis.com/css2?family=Orbitron:wght@500;800&family=JetBrains+Mono:wght@400;700&display=swap";

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(Shell)]
    #[route("/")]
    Terminal {},
    #[route("/checkout/:invoice_id")]
    Checkout { invoice_id: String },
    #[route("/exchange")]
    Swap {},
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        document::Link { rel: "stylesheet", href: FONTS }
        document::Link { rel: "stylesheet", href: CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    let health = use_resource(api::health);
    let (chip, txt) = match &*health.read() {
        Some(Ok(true)) => ("chip online", "BLACKVAULT // LINKED"),
        Some(_) => ("chip offline", "BLACKVAULT // DOWN"),
        None => ("chip", "BLACKVAULT // …"),
    };

    rsx! {
        div { class: "grid-bg" }
        header { class: "topbar",
            h1 { class: "logo glitch", "data-text": "DTAP", "DTAP" }
            span { class: chip, "{txt}" }
        }
        main { class: "stage", Outlet::<Route> {} }
        nav { class: "tabbar",
            Link { to: Route::Terminal {}, class: "tab", active_class: "active", "▣ TERMINAL" }
            Link { to: Route::Swap {}, class: "tab", active_class: "active", "⇄ EXCHANGE LAB" }
        }
    }
}
