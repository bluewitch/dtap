use dtap_core::*;
use serde::{de::DeserializeOwned, Deserialize};

// Build-time config: DTAP_GATEWAY_URL=https://... dx serve
const GATEWAY: &str = match option_env!("DTAP_GATEWAY_URL") {
    Some(u) => u,
    None => "http://127.0.0.1:8787",
};
const TOKEN: &str = match option_env!("DTAP_TERMINAL_TOKEN") {
    Some(t) => t,
    None => "dev-terminal-token",
};

async fn decode<T: DeserializeOwned>(r: reqwest::Response) -> Result<T, String> {
    let status = r.status();
    if !status.is_success() {
        return Err(format!("{status}: {}", r.text().await.unwrap_or_default()));
    }
    r.json().await.map_err(|e| e.to_string())
}

async fn get<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let r = reqwest::Client::new()
        .get(format!("{GATEWAY}{path}"))
        .bearer_auth(TOKEN)
        .send().await.map_err(|e| e.to_string())?;
    decode(r).await
}

async fn post<B: serde::Serialize, T: DeserializeOwned>(path: &str, body: &B) -> Result<T, String> {
    let r = reqwest::Client::new()
        .post(format!("{GATEWAY}{path}"))
        .bearer_auth(TOKEN)
        .json(body)
        .send().await.map_err(|e| e.to_string())?;
    decode(r).await
}

#[derive(Deserialize)]
struct Health { blackvault: bool }

pub async fn health() -> Result<bool, String> {
    get::<Health>("/health").await.map(|h| h.blackvault)
}
pub async fn create_charge(req: &CreateCharge) -> Result<Charge, String> {
    post("/v1/charges", req).await
}
pub async fn get_charge(id: &str) -> Result<Charge, String> {
    get(&format!("/v1/charges/{id}")).await
}
pub async fn get_status(id: &str) -> Result<ChargeStatus, String> {
    get(&format!("/v1/charges/{id}/status")).await
}
pub async fn tap_pay(req: &TapRequest) -> Result<serde_json::Value, String> {
    post("/v1/tap", req).await
}
pub async fn quote(from: &str, to: &str) -> Result<SwapQuote, String> {
    get(&format!("/v1/swap/quote/{from}/{to}")).await
}
