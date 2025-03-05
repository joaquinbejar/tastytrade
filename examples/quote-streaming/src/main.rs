use tastytrade_rs::dxfeed;
use tastytrade_rs::TastyTrade;
use dxfeed::EventData::Quote;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let username = args.next().unwrap();
    let password = args.next().unwrap();

    let tasty = TastyTrade::login(&username, &password, false)
        .await
        .unwrap();

    let streamer = tasty.create_quote_streamer().await.unwrap();
    streamer.subscribe(&["SPX"]);

    while let Ok(ev) = streamer.get_event().await {
        if let Quote(data) = ev.data {
            println!("{}: {}/{}", ev.sym, data.bid_price, data.ask_price);
        }
    }
}
