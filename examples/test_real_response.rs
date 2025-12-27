//! Test deserialization of real API response from user

use calchas::kalshi::types::MarketsResponse;

fn main() {
    // Real API response from user
    let json = r#"{"cursor":"CgwI18S9ygYQ4KS5lAMSOUtYTVZFU1BPUlRTTVVMVElHQU1FRVhURU5ERUQtUzIwMjVDRDFFMTlCNjZBMC1FRUI3QkNEQzA1QQ","markets":[{"can_close_early":true,"category":"","close_time":"2026-01-10T02:30:00Z","created_time":"2025-12-27T04:36:49.776769Z","custom_strike":{"Associated Events":"KXNBAAST-25DEC26DETUTA,KXNBAPTS-25DEC26DETUTA","Associated Market Sides":"yes,yes","Associated Markets":"KXNBAAST-25DEC26DETUTA-UTALMARKKANEN23-4,KXNBAPTS-25DEC26DETUTA-DETCCUNNINGHAM2-25","Multivariate Event Ticker":"KXMVESPORTSMULTIGAMEEXTENDED-R"},"event_ticker":"KXMVESPORTSMULTIGAMEEXTENDED-S2025B00AF9F365C","expected_expiration_time":"2025-12-27T06:00:00Z","expiration_time":"2026-01-10T02:30:00Z","expiration_value":"","last_price":0,"last_price_dollars":"0.0000","latest_expiration_time":"2026-01-10T02:30:00Z","liquidity":0,"liquidity_dollars":"0.0000","market_type":"binary","mve_collection_ticker":"KXMVESPORTSMULTIGAMEEXTENDED-R","mve_selected_legs":[{"event_ticker":"KXNBAAST-25DEC26DETUTA","market_ticker":"KXNBAAST-25DEC26DETUTA-UTALMARKKANEN23-4","side":"yes"}],"no_ask":100,"no_ask_dollars":"1.0000","no_bid":100,"no_bid_dollars":"1.0000","no_sub_title":"test","notional_value":100,"notional_value_dollars":"1.0000","open_interest":0,"open_time":"2025-12-27T04:36:49.739293Z","previous_price":0,"previous_price_dollars":"0.0000","previous_yes_ask":0,"previous_yes_ask_dollars":"0.0000","previous_yes_bid":0,"previous_yes_bid_dollars":"0.0000","price_level_structure":"deci_cent","price_ranges":[{"end":"1.0000","start":"0.0000","step":"0.0010"}],"response_price_units":"usd_cent","result":"","risk_limit_cents":0,"rules_primary":"","rules_secondary":"","settlement_timer_seconds":600,"status":"active","strike_type":"custom","subtitle":"","tick_size":1,"ticker":"TEST-TICKER","title":"Test Market","volume":0,"volume_24h":0,"yes_ask":0,"yes_ask_dollars":"0.0000","yes_bid":0,"yes_bid_dollars":"0.0000","yes_sub_title":"test"}]}"#;

    match serde_json::from_str::<MarketsResponse>(json) {
        Ok(response) => {
            println!("✓ Deserialization SUCCEEDED");
            println!("  Cursor: {:?}", response.cursor);
            println!("  Markets: {}", response.markets.len());
            if let Some(market) = response.markets.first() {
                println!("  First market ticker: {}", market.ticker);
                println!("  Status: {}", market.status);
                println!("  Category: {:?}", market.category);
            }
        }
        Err(e) => {
            println!("✗ Deserialization FAILED");
            println!("  Error: {}", e);
            println!();
            println!("This means our struct is missing required fields!");
        }
    }
}
