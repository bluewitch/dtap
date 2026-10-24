use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use dtap_core::{Charge, ChargeStatus, CreateCharge, Rail, TapRequest};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tower_http::cors::CorsLayer;

const DEFAULT_BIND: &str = "127.0.0.1:8787";
const DEFAULT_TOKEN: &str = "dev-terminal-token";
const DEFAULT_ORIGINS: &str = "http://127.0.0.1:8080,http://localhost:8080";
/// Round demo price so a $5 sale is 5,000 sats. Labeled demo in the rate string.
const DEMO_USD_PER_BTC: u64 = 100_000;
const SATS_PER_BTC: u64 = 100_000_000;

#[derive(Clone)]
struct AppState {
    mock: bool,
    blackvault: bool,
    token: String,
    charges: Arc<Mutex<HashMap<String, StoredCharge>>>,
}

#[derive(Clone)]
struct StoredCharge {
    charge: Charge,
    created: Instant,
}

struct Mode {
    mock: bool,
    blackvault: bool,
    creds_present: bool,
}

#[derive(Deserialize)]
struct StatusQuery {
    status: Option<String>,
}

fn is_truthy(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn env_nonempty(key: &str) -> bool {
    std::env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn btcpay_creds_present() -> bool {
    ["BTCPAY_URL", "BTCPAY_API_KEY", "BTCPAY_STORE_ID"]
        .into_iter()
        .all(env_nonempty)
}

/// Real BTCPay is not wired. This stays false so /health cannot claim a live link.
fn btcpay_client_connected() -> bool {
    false
}

fn blackvault_live(mock: bool, creds_present: bool) -> bool {
    if mock || !creds_present {
        return false;
    }
    btcpay_client_connected()
}

/// `DTAP_MOCK=1` forces mock. When the flag is unset, missing BTCPay credentials also select mock.
/// `DTAP_MOCK=0` is non-mock even with no credentials; blackvault stays false until a client exists.
fn mode_from(dtap_mock: Option<&str>, creds_present: bool) -> Mode {
    let mock = match dtap_mock {
        Some(value) => is_truthy(value),
        None => !creds_present,
    };
    Mode {
        mock,
        blackvault: blackvault_live(mock, creds_present),
        creds_present,
    }
}

fn load_mode() -> Mode {
    let flag = std::env::var("DTAP_MOCK").ok();
    mode_from(flag.as_deref(), btcpay_creds_present())
}

fn env_or_default(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default.to_string(),
    }
}

fn bind_addr() -> String {
    env_or_default("DTAP_BIND", DEFAULT_BIND)
}

fn terminal_token() -> String {
    env_or_default("DTAP_TERMINAL_TOKEN", DEFAULT_TOKEN)
}

fn cors_origins(configured: &str) -> Vec<HeaderValue> {
    let mut origins: Vec<HeaderValue> = configured
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .filter_map(|origin| origin.parse().ok())
        .collect();
    if origins.is_empty() {
        origins.push(HeaderValue::from_static("http://127.0.0.1:8080"));
        origins.push(HeaderValue::from_static("http://localhost:8080"));
    }
    origins
}

fn cors_layer() -> CorsLayer {
    let configured = env_or_default("DTAP_CORS_ORIGINS", DEFAULT_ORIGINS);
    CorsLayer::new()
        .allow_origin(cors_origins(&configured))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static("x-dtap-mock-status"),
        ])
}

fn bearer_matches(headers: &HeaderMap, expected: &str) -> bool {
    if expected.is_empty() {
        return false;
    }
    let Some(raw) = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some((scheme, token)) = raw.split_once(' ') else {
        return false;
    };
    scheme.eq_ignore_ascii_case("bearer") && token.trim() == expected
}

async fn require_bearer(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if request.method() == Method::OPTIONS {
        return next.run(request).await;
    }
    if bearer_matches(request.headers(), &state.token) {
        next.run(request).await
    } else {
        error_response(StatusCode::UNAUTHORIZED, "unauthorized")
    }
}

fn error_response(status: StatusCode, error: &str) -> Response {
    (status, Json(serde_json::json!({ "error": error }))).into_response()
}

fn json_error(err: JsonRejection) -> Response {
    (
        err.status(),
        Json(serde_json::json!({ "error": err.body_text() })),
    )
        .into_response()
}

fn usd_cents(amount: &str) -> u64 {
    let trimmed = amount.trim().trim_end_matches('.');
    if trimmed.is_empty() {
        return 0;
    }
    let (whole, frac) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
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

fn demo_sats(amount: &str) -> u64 {
    let cents = usd_cents(amount);
    cents.saturating_mul(SATS_PER_BTC) / (DEMO_USD_PER_BTC.saturating_mul(100))
}

fn group_digits(value: u64) -> String {
    let raw = value.to_string();
    let mut out = String::new();
    for (i, ch) in raw.chars().enumerate() {
        if i > 0 && (raw.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn demo_suffix(id: &str) -> String {
    let hex: String = id
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(12)
        .collect();
    if hex.is_empty() {
        "invoice".to_string()
    } else {
        hex
    }
}

fn demo_charge(req: &CreateCharge, id: &str) -> Charge {
    let amount_due = format!("{} sats", group_digits(demo_sats(&req.amount)));
    let rate = format!("{DEMO_USD_PER_BTC} USD/BTC demo");
    let suffix = demo_suffix(id);
    let lightning = format!("lnbc1demo{suffix}");
    let onchain = format!("bc1qdemo{suffix}");
    Charge {
        invoice_id: id.to_string(),
        amount: req.amount.clone(),
        currency: req.currency.clone(),
        checkout_link: String::new(),
        status: ChargeStatus::New,
        rails: vec![
            Rail {
                id: "BTC-LN".into(),
                destination: format!("{lightning} (demo)"),
                payment_link: Some(format!("dtap-demo:lightning:{lightning}")),
                amount_due: amount_due.clone(),
                rate: rate.clone(),
            },
            Rail {
                id: "BTC".into(),
                destination: format!("{onchain} (demo)"),
                payment_link: Some(format!("dtap-demo:bitcoin:{onchain}")),
                amount_due,
                rate,
            },
        ],
    }
}

fn parse_status(raw: &str) -> Result<ChargeStatus, ()> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "new" => Ok(ChargeStatus::New),
        "processing" => Ok(ChargeStatus::Processing),
        "settled" => Ok(ChargeStatus::Settled),
        "expired" => Ok(ChargeStatus::Expired),
        "invalid" => Ok(ChargeStatus::Invalid),
        _ => Err(()),
    }
}

fn status_after(elapsed: Duration) -> ChargeStatus {
    if elapsed < Duration::from_secs(3) {
        ChargeStatus::New
    } else if elapsed < Duration::from_secs(8) {
        ChargeStatus::Processing
    } else {
        ChargeStatus::Settled
    }
}

fn requested_status(query: &StatusQuery, headers: &HeaderMap) -> Option<String> {
    let from_query = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if from_query.is_some() {
        return from_query;
    }
    headers
        .get("x-dtap-mock-status")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn resolve_status(created: Instant, override_status: Option<&str>) -> Result<ChargeStatus, ()> {
    match override_status {
        Some(raw) => parse_status(raw),
        None => Ok(status_after(created.elapsed())),
    }
}

enum StatusError {
    NotFound,
    MockOnly,
    Unknown,
}

fn status_error(err: StatusError) -> Response {
    match err {
        StatusError::NotFound => error_response(StatusCode::NOT_FOUND, "not found"),
        StatusError::MockOnly => error_response(
            StatusCode::BAD_REQUEST,
            "status override requires mock mode",
        ),
        StatusError::Unknown => error_response(StatusCode::BAD_REQUEST, "unknown status"),
    }
}

fn lookup_status(
    state: &AppState,
    id: &str,
    query: &StatusQuery,
    headers: &HeaderMap,
) -> Result<(Charge, ChargeStatus), StatusError> {
    let stored = {
        let map = state.charges.lock().unwrap();
        map.get(id).cloned()
    };
    let Some(stored) = stored else {
        return Err(StatusError::NotFound);
    };
    let requested = requested_status(query, headers);
    if requested.is_some() && !state.mock {
        return Err(StatusError::MockOnly);
    }
    match resolve_status(stored.created, requested.as_deref()) {
        Ok(status) => Ok((stored.charge, status)),
        Err(()) => Err(StatusError::Unknown),
    }
}

fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/charges", post(create_charge))
        .route("/v1/charges/{id}", get(get_charge))
        .route("/v1/charges/{id}/status", get(get_status))
        .route("/v1/tap", post(tap))
        .route("/v1/swap/quote/{from}/{to}", get(quote))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer,
        ))
        .layer(cors_layer())
        .with_state(state)
}

#[tokio::main]
async fn main() {
    let mode = load_mode();
    let bind = bind_addr();
    let state = AppState {
        mock: mode.mock,
        blackvault: mode.blackvault,
        token: terminal_token(),
        charges: Arc::new(Mutex::new(HashMap::new())),
    };
    let app = router(state);
    let creds = if mode.creds_present {
        "present"
    } else {
        "absent"
    };
    println!(
        "dtap-gateway listening on {bind} (mock={} blackvault={} btcpay_creds={creds})",
        mode.mock, mode.blackvault
    );
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|err| panic!("failed to bind {bind}: {err}"));
    axum::serve(listener, app)
        .await
        .unwrap_or_else(|err| panic!("dtap-gateway stopped: {err}"));
}

async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "blackvault": state.blackvault,
        "mock": state.mock,
    }))
}

async fn create_charge(
    State(state): State<AppState>,
    body: Result<Json<CreateCharge>, JsonRejection>,
) -> Response {
    let Json(req) = match body {
        Ok(json) => json,
        Err(err) => return json_error(err),
    };
    if !state.mock {
        return error_response(StatusCode::NOT_IMPLEMENTED, "real BTCPay not wired");
    }
    let id = uuid::Uuid::new_v4().to_string();
    let charge = demo_charge(&req, &id);
    state.charges.lock().unwrap().insert(
        id,
        StoredCharge {
            charge: charge.clone(),
            created: Instant::now(),
        },
    );
    (StatusCode::CREATED, Json(charge)).into_response()
}

async fn get_charge(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<StatusQuery>,
    headers: HeaderMap,
) -> Response {
    match lookup_status(&state, &id, &query, &headers) {
        Ok((mut charge, status)) => {
            charge.status = status;
            Json(charge).into_response()
        }
        Err(err) => status_error(err),
    }
}

async fn get_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<StatusQuery>,
    headers: HeaderMap,
) -> Response {
    match lookup_status(&state, &id, &query, &headers) {
        Ok((_, status)) => Json(status).into_response(),
        Err(err) => status_error(err),
    }
}

async fn tap(body: Result<Json<TapRequest>, JsonRejection>) -> Response {
    if let Err(err) = body {
        return json_error(err);
    }
    error_response(StatusCode::NOT_IMPLEMENTED, "tap not configured")
}

async fn quote(Path((_from, _to)): Path<(String, String)>) -> Response {
    error_response(StatusCode::NOT_IMPLEMENTED, "swap not configured")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_flag_forces_demo_even_with_creds() {
        let mode = mode_from(Some("1"), true);
        assert!(mode.mock);
        assert!(!mode.blackvault);
        assert!(mode_from(Some("TRUE"), false).mock);
    }

    #[test]
    fn missing_creds_default_to_mock() {
        let mode = mode_from(None, false);
        assert!(mode.mock);
        assert!(!mode.blackvault);
    }

    #[test]
    fn creds_without_flag_are_not_mock_and_not_linked() {
        let mode = mode_from(None, true);
        assert!(!mode.mock);
        assert!(!mode.blackvault);
    }

    #[test]
    fn non_mock_without_creds_does_not_claim_blackvault() {
        let mode = mode_from(Some("0"), false);
        assert!(!mode.mock);
        assert!(!mode.blackvault);
        let with_creds = mode_from(Some("false"), true);
        assert!(!with_creds.mock);
        assert!(!with_creds.blackvault);
    }

    #[test]
    fn five_dollars_is_five_thousand_demo_sats() {
        assert_eq!(demo_sats("5"), 5_000);
        assert_eq!(demo_sats("5.00"), 5_000);
        assert_eq!(demo_sats("5."), 5_000);
        assert_eq!(demo_sats("1.50"), 1_500);
        assert_eq!(demo_sats("0.01"), 10);
    }

    #[test]
    fn demo_charge_is_labeled_and_has_no_checkout_host() {
        let charge = demo_charge(
            &CreateCharge {
                amount: "5".into(),
                currency: "USD".into(),
                memo: None,
            },
            "550e8400-e29b-41d4-a716-446655440000",
        );
        assert!(charge.checkout_link.is_empty());
        assert_eq!(charge.rails.len(), 2);
        assert_eq!(charge.rails[0].id, "BTC-LN");
        assert_eq!(charge.rails[0].amount_due, "5,000 sats");
        assert!(charge.rails[0].rate.contains("demo"));
        assert!(charge.rails[0].destination.contains("(demo)"));
        let link = charge.rails[0].payment_link.as_deref().unwrap_or("");
        assert!(link.starts_with("dtap-demo:lightning:"));
        assert!(
            charge.rails[1]
                .payment_link
                .as_deref()
                .unwrap_or("")
                .contains("demo")
        );
        assert!(!format!("{charge:?}").to_ascii_lowercase().contains("txid"));
    }

    #[test]
    fn unknown_status_is_rejected() {
        assert!(parse_status("nope").is_err());
        assert!(parse_status("paid").is_err());
        assert_eq!(parse_status("settled").unwrap(), ChargeStatus::Settled);
        assert_eq!(
            parse_status("Processing").unwrap(),
            ChargeStatus::Processing
        );
    }

    #[test]
    fn status_timeline() {
        assert_eq!(status_after(Duration::from_secs(0)), ChargeStatus::New);
        assert_eq!(status_after(Duration::from_secs(2)), ChargeStatus::New);
        assert_eq!(
            status_after(Duration::from_secs(3)),
            ChargeStatus::Processing
        );
        assert_eq!(
            status_after(Duration::from_secs(7)),
            ChargeStatus::Processing
        );
        assert_eq!(status_after(Duration::from_secs(8)), ChargeStatus::Settled);
    }

    #[test]
    fn bearer_matches_only_the_configured_token() {
        let mut headers = HeaderMap::new();
        assert!(!bearer_matches(&headers, DEFAULT_TOKEN));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer nope"),
        );
        assert!(!bearer_matches(&headers, DEFAULT_TOKEN));
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer dev-terminal-token"),
        );
        assert!(bearer_matches(&headers, DEFAULT_TOKEN));
        assert!(!bearer_matches(&headers, ""));
    }

    #[test]
    fn default_cors_origins_are_the_dev_servers() {
        let origins = cors_origins(DEFAULT_ORIGINS);
        let text: Vec<String> = origins
            .iter()
            .map(|origin| origin.to_str().unwrap().to_string())
            .collect();
        assert_eq!(
            text,
            vec![
                "http://127.0.0.1:8080".to_string(),
                "http://localhost:8080".to_string()
            ]
        );
    }
}
