use tastytrade_rs::TastyTrade;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let username = args.next().unwrap();
    let password = args.next().unwrap();

    let tasty = TastyTrade::login(&username, &password, false)
        .await
        .unwrap();

    let accounts = tasty.accounts().await.unwrap();
    for account in accounts {
        println!("Account: {}", account.number().0);
        println!(
            "Positions in: {:?}",
            account
                .positions()
                .await
                .unwrap()
                .into_iter()
                .map(|p| p.symbol.0)
                .collect::<Vec<String>>()
        )
    }
}
