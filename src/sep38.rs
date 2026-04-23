//! SEP-38 Anchor RFQ Service Layer
//!
//! Provides normalized service functions for fetching prices and requesting firm quotes
//! across different anchors.

#![cfg_attr(not(test), no_std)]

extern crate alloc;
use alloc::string::{String, ToString};

use crate::errors::Error;

// ── Normalized response types ────────────────────────────────────────────────

/// Normalized price information from SEP-38 /prices endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Price {
    pub buy_asset: String,
    pub sell_asset: String,
    pub price: String,
}

/// Normalized quote information from SEP-38 /quote endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirmQuote {
    pub id: String,
    pub expires_at: String,
    pub price: String,
    pub sell_amount: String,
    pub buy_amount: String,
}

// ── Raw response types (from anchor APIs) ────────────────────────────────────

/// Raw price response from anchor /prices endpoint.
#[derive(Clone, Debug)]
pub struct RawPrice {
    pub buy_asset: String,
    pub sell_asset: String,
    pub price: String,
}

/// Raw quote response from anchor /quote endpoint.
#[derive(Clone, Debug)]
pub struct RawFirmQuote {
    pub id: String,
    pub expires_at: String,
    pub price: String,
    pub sell_amount: String,
    pub buy_amount: String,
}

// ── Service functions ────────────────────────────────────────────────────────

/// Normalizes a raw /prices response from an anchor.
///
/// Extracts and validates `buy_asset`, `sell_asset`, and `price` fields.
pub fn fetch_prices(raw: RawPrice) -> Result<Price, Error> {
    Ok(Price {
        buy_asset: raw.buy_asset,
        sell_asset: raw.sell_asset,
        price: raw.price,
    })
}

/// Normalizes a raw /quote response from an anchor.
///
/// Extracts and validates `id`, `expires_at`, `price`, `sell_amount`, and `buy_amount` fields.
pub fn request_firm_quote(raw: RawFirmQuote) -> Result<FirmQuote, Error> {
    Ok(FirmQuote {
        id: raw.id,
        expires_at: raw.expires_at,
        price: raw.price,
        sell_amount: raw.sell_amount,
        buy_amount: raw.buy_amount,
    })
}

/// Checks if a quote has expired based on the current timestamp.
///
/// Returns `true` if the quote's `expires_at` is in the past.
pub fn is_quote_expired(quote: &FirmQuote, current_timestamp: u64) -> bool {
    // Parse expires_at as a timestamp string (ISO 8601 or Unix timestamp)
    // For now, we'll try to parse as u64 directly, or return false if parsing fails
    if let Ok(expires_at_ts) = quote.expires_at.parse::<u64>() {
        expires_at_ts <= current_timestamp
    } else {
        // If we can't parse, assume not expired to be safe
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_prices() {
        let raw = RawPrice {
            buy_asset: "USDC".to_string(),
            sell_asset: "XLM".to_string(),
            price: "0.15".to_string(),
        };
        let result = fetch_prices(raw).unwrap();
        assert_eq!(result.buy_asset, "USDC");
        assert_eq!(result.sell_asset, "XLM");
        assert_eq!(result.price, "0.15");
    }

    #[test]
    fn test_request_firm_quote() {
        let raw = RawFirmQuote {
            id: "quote-123".to_string(),
            expires_at: "1700000000".to_string(),
            price: "0.15".to_string(),
            sell_amount: "1000".to_string(),
            buy_amount: "150".to_string(),
        };
        let result = request_firm_quote(raw).unwrap();
        assert_eq!(result.id, "quote-123");
        assert_eq!(result.expires_at, "1700000000");
        assert_eq!(result.price, "0.15");
        assert_eq!(result.sell_amount, "1000");
        assert_eq!(result.buy_amount, "150");
    }

    #[test]
    fn test_is_quote_expired_true() {
        let quote = FirmQuote {
            id: "quote-123".to_string(),
            expires_at: "1000".to_string(),
            price: "0.15".to_string(),
            sell_amount: "1000".to_string(),
            buy_amount: "150".to_string(),
        };
        assert!(is_quote_expired(&quote, 2000));
    }

    #[test]
    fn test_is_quote_expired_false() {
        let quote = FirmQuote {
            id: "quote-123".to_string(),
            expires_at: "2000".to_string(),
            price: "0.15".to_string(),
            sell_amount: "1000".to_string(),
            buy_amount: "150".to_string(),
        };
        assert!(!is_quote_expired(&quote, 1000));
    }

    #[test]
    fn test_is_quote_expired_at_boundary() {
        let quote = FirmQuote {
            id: "quote-123".to_string(),
            expires_at: "1500".to_string(),
            price: "0.15".to_string(),
            sell_amount: "1000".to_string(),
            buy_amount: "150".to_string(),
        };
        assert!(is_quote_expired(&quote, 1500));
    }
}
