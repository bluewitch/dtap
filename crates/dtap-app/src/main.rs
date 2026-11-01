use dioxus::prelude::*;

mod api;
mod config;
mod nfc;
mod views;
use views::{Checkout, Pair, Swap, Terminal};

const CSS: Asset = asset!("/assets/cyber.css");

#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(Shell)]
    #[route("/")]
    Terminal {},
    #[route("/checkout/:invoice_id")]
    Checkout { invoice_id: String },
    #[route("/exchange")]
    Swap {},
    #[route("/pair")]
    Pair {},
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        document::Meta { name: "viewport", content: "width=device-width, initial-scale=1, viewport-fit=cover" }
        document::Link { rel: "stylesheet", href: CSS }
        Router::<Route> {}
    }
}

#[component]
fn Shell() -> Element {
    let health = use_resource(api::health);
    let (chip, txt) = match &*health.read() {
        Some(Ok(h)) if h.mock => ("chip demo", "DEMO MODE"),
        Some(Ok(h)) if h.blackvault => ("chip online", "BLACKVAULT // LINKED"),
        Some(Ok(_)) | Some(Err(_)) => ("chip offline", "BLACKVAULT // DOWN"),
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
            Link { to: Route::Pair {}, class: "tab", active_class: "active",
                if config::current().paired { "⚙ PAIRED" } else { "⚙ PAIR" }
            }
        }
    }
}
