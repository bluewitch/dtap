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
    fn up(&self) -> String {
        self.id.to_ascii_uppercase()
    }

    pub fn is_lightning(&self) -> bool {
        let u = self.up();
        u == "BTC-LN" || u.contains("LIGHTNING")
    }

    /// Only Lightning and on-chain Bitcoin are supported. Any other rail id a backend
    /// might return (Monero, Liquid, stablecoins, ...) is shown as unsupported, never by name.
    pub fn css(&self) -> &'static str {
        let u = self.up();
        if self.is_lightning() || u.contains("LNURL") {
            "ln"
        } else if u == "BTC" || u == "BTC-CHAIN" || u == "BTC-ONCHAIN" {
            "btc"
        } else {
            "alt"
        }
    }

    pub fn label(&self) -> &'static str {
        match self.css() {
            "ln" => "⚡ LIGHTNING",
            "btc" => "₿ ON-CHAIN",
            _ => "◇ UNSUPPORTED RAIL",
        }
    }
}

#[cfg(test)]
mod rail_tests {
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
    fn only_bitcoin_rails_are_named() {
        assert_eq!(rail("BTC-LN").label(), "⚡ LIGHTNING");
        assert_eq!(rail("BTC").label(), "₿ ON-CHAIN");
        for id in ["XMR", "LBTC", "USDT", "USDC", "BTC-LIQUID", "LTC"] {
            assert_eq!(rail(id).css(), "alt", "{id}");
            assert_eq!(rail(id).label(), "◇ UNSUPPORTED RAIL", "{id}");
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum ChargeStatus {
    New,
    Processing,
    Settled,
    Expired,
    Invalid,
}

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
            Self::New => "new",
            Self::Processing => "processing",
            Self::Settled => "settled",
            Self::Expired => "expired",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TapRequest {
    pub invoice_id: String,
    pub lnurlw: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SwapQuote {
    pub from: String,
    pub to: String,
    pub rate: String,
    pub min: String,
    pub max: String,
}
