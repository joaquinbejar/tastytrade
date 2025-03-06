use dxfeed::EventData::Quote;
use tastytrade_rs::TastyTrade;
use tastytrade_rs::dxfeed;
use tastytrade_rs::utils::config::Config;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    let tasty = TastyTrade::login(&config).await.unwrap();

    let streamer = tasty.create_quote_streamer().await.unwrap();
    streamer.subscribe(&["SPX"]);

    while let Ok(ev) = streamer.get_event().await {
        if let Quote(data) = ev.data {
            println!("{}: {}/{}", ev.sym, data.bid_price, data.ask_price);
        }
    }
}
