use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use dtap_core::{Charge, ChargeStatus, CreateCharge, Rail, SwapQuote, TapRequest};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    mock: bool,
    charges: Arc<Mutex<HashMap<String, StoredCharge>>>,
}

struct StoredCharge {
    charge: Charge,
    created: Instant,
}

#[derive(Deserialize)]
struct StatusQuery {
    status: Option<String>,
}

#[tokio::main]
async fn main() {
    let mock = std::env::var("DTAP_MOCK").map(|v| v == "1" || v.eq_ignore_ascii_case("true")).unwrap_or(true);
    let state = AppState {
        mock,
        charges: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/charges", post(create_charge))
        .route("/v1/charges/{id}", get(get_charge))
        .route("/v1/charges/{id}/status", get(get_status))
        .route("/v1/tap", post(tap))
        .route("/v1/swap/quote/{from}/{to}", get(quote))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = "0.0.0.0:8787";
    println!("dtap-gateway listening on {addr} (mock={mock})");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "blackvault": !state.mock,
        "mock": state.mock
    }))
}

async fn create_charge(
    State(state): State<AppState>,
    Json(req): Json<CreateCharge>,
) -> impl IntoResponse {
    if !state.mock {
        return (StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({"error": "real BTCPay not wired"}))).into_response();
    }

    let id = uuid::Uuid::new_v4().to_string();
    let charge = Charge {
        invoice_id: id.clone(),
        amount: req.amount.clone(),
        currency: req.currency.clone(),
        checkout_link: format!("https://demo.dtap.local/i/{id}"),
        status: ChargeStatus::New,
        rails: vec![
            Rail {
                id: "BTC-LN".into(),
                destination: format!("lnbc{id}"),
                payment_link: Some(format!("lightning:lnbc{id}")),
                amount_due: req.amount.clone(),
                rate: "1".into(),
            },
            Rail {
                id: "BTC".into(),
                destination: format!("bc1qdemo{id}"),
                payment_link: None,
                amount_due: req.amount.clone(),
                rate: "1".into(),
            },
        ],
    };

    state.charges.lock().unwrap().insert(id.clone(), StoredCharge {
        charge: charge.clone(),
        created: Instant::now(),
    });

    (StatusCode::CREATED, Json(charge)).into_response()
}

fn resolve_status(stored: &StoredCharge, override_status: Option<&str>) -> ChargeStatus {
    if let Some(s) = override_status {
        return match s.to_ascii_lowercase().as_str() {
            "processing" => ChargeStatus::Processing,
            "settled" => ChargeStatus::Settled,
            "expired" => ChargeStatus::Expired,
            "invalid" => ChargeStatus::Invalid,
            _ => ChargeStatus::New,
        };
    }
    let elapsed = stored.created.elapsed();
    if elapsed < Duration::from_secs(3) {
        ChargeStatus::New
    } else if elapsed < Duration::from_secs(8) {
        ChargeStatus::Processing
    } else {
        ChargeStatus::Settled
    }
}

async fn get_charge(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<StatusQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let mut map = state.charges.lock().unwrap();
    let Some(stored) = map.get_mut(&id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response();
    };
    let override_status = q.status.as_deref().or_else(|| headers.get("x-dtap-mock-status").and_then(|v| v.to_str().ok()));
    stored.charge.status = resolve_status(stored, override_status);
    Json(stored.charge.clone()).into_response()
}

async fn get_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<StatusQuery>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let map = state.charges.lock().unwrap();
    let Some(stored) = map.get(&id) else {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))).into_response();
    };
    let override_status = q.status.as_deref().or_else(|| headers.get("x-dtap-mock-status").and_then(|v| v.to_str().ok()));
    let status = resolve_status(stored, override_status);
    Json(status).into_response()
}

async fn tap(Json(_req): Json<TapRequest>) -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({"error": "tap not configured"})),
    )
}

async fn quote(Path((from, to)): Path<(String, String)>) -> impl IntoResponse {
    let _ = (from, to);
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(serde_json::json!({"error": "swap not configured"})),
    )
}
