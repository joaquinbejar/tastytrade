use tastytrade::TastyTrade;
use tastytrade::utils::config::Config;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    let tasty = TastyTrade::login(&config).await.unwrap();

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
