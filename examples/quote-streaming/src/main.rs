use dxfeed::EventData::Quote;
use tastytrade::TastyTrade;
use tastytrade::dxfeed;
use tastytrade::utils::config::Config;
use tastytrade::utils::logger::setup_logger;

#[tokio::main]
async fn main() {
    setup_logger();
    let config = Config::new();
    let tasty = TastyTrade::login(&config).await.unwrap();

    let streamer = tasty.create_quote_streamer().await.unwrap();
    streamer.subscribe(&["SPX"]);

    while let Ok(ev) = streamer.get_event().await {
        if let Quote(data) = ev.data {
            tracing::info!("{}: {}/{}", ev.sym, data.bid_price, data.ask_price);
        }
    }
}
