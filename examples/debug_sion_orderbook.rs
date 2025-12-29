//! Debug Sion orderbook
use calchas::config::AppConfig;
use calchas::kalshi::client::KalshiClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_target(false).init();

    let config = AppConfig::load_with_env_default()?;
    let client = KalshiClient::from_config(&config.kalshi)?;

    let response = client.get_orderbook("KXSWISSLEAGUEGAME-25DEC20SIOWIN-SIO", None).await?;

    if let Some(data) = response.orderbook {
        println!("\n=== YES SIDE (full orderbook) ===");
        for (i, (price, qty)) in data.yes.iter().enumerate() {
            println!("[{}] {}¢ - {} contracts", i, price, qty);
        }

        println!("\n=== NO SIDE (full orderbook) ===");
        for (i, (price, qty)) in data.no.iter().enumerate() {
            println!("[{}] {}¢ - {} contracts", i, price, qty);
        }

        println!("\n=== ANALYSIS ===");
        println!("YES .first() = {}¢", data.yes.first().map(|(p,_)| p).unwrap_or(&0));
        println!("YES .last() = {}¢", data.yes.last().map(|(p,_)| p).unwrap_or(&0));
        println!("NO .first() = {}¢", data.no.first().map(|(p,_)| p).unwrap_or(&0));
        println!("NO .last() = {}¢", data.no.last().map(|(p,_)| p).unwrap_or(&0));
    } else {
        println!("No orderbook data!");
    }

    Ok(())
}
