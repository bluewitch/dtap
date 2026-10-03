use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CreateCharge {
    pub amount: String,
    pub currency: String,
    pub memo: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Charge {
    pub invoice_id: String,
    pub amount: String,
    pub currency: String,
    pub checkout_link: String,
    pub status: ChargeStatus,
    pub rails: Vec<Rail>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rail {
    pub id: String,
    pub destination: String,
    pub payment_link: Option<String>,
    pub amount_due: String,
    pub rate: String,
}

impl Rail {
    fn up(&self) -> String { self.id.to_ascii_uppercase() }

    pub fn is_lightning(&self) -> bool {
        let u = self.up();
        u == "BTC-LN" || u.contains("LIGHTNING")
    }

    pub fn css(&self) -> &'static str {
        let u = self.up();
        if u.contains("USDT") || u.contains("USDC") { "stable" }
        else if self.is_lightning() || u.contains("LNURL") { "ln" }
        else if u.starts_with("XMR") { "xmr" }
        else if u.starts_with("LBTC") { "liquid" }
        else if u.starts_with("BTC") { "btc" }
        else { "alt" }
    }

    pub fn label(&self) -> &'static str {
        match self.css() {
            "stable" => "◈ STABLECOIN",
            "ln" => "⚡ LIGHTNING",
            "xmr" => "ɱ MONERO",
            "liquid" => "◆ LIQUID",
            "btc" => "₿ ON-CHAIN",
            _ => "◇ ALT RAIL",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ChargeStatus { New, Processing, Settled, Expired, Invalid }

impl ChargeStatus {
    pub fn from_btcpay(s: &str) -> Self {
        match s {
            "Processing" => Self::Processing,
            "Settled" => Self::Settled,
            "Expired" => Self::Expired,
            "Invalid" => Self::Invalid,
            _ => Self::New,
        }
    }
    pub fn is_final(self) -> bool {
        matches!(self, Self::Settled | Self::Expired | Self::Invalid)
    }
    pub fn css(self) -> &'static str {
        match self {
            Self::New => "new", Self::Processing => "processing",
            Self::Settled => "settled", Self::Expired => "expired", Self::Invalid => "invalid",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TapRequest { pub invoice_id: String, pub lnurlw: String }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SwapQuote { pub from: String, pub to: String, pub rate: String, pub min: String, pub max: String }
