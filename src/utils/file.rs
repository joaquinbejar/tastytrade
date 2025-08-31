/******************************************************************************
   Author: Joaquín Béjar García
   Email: jb@taunais.com
   Date: 31/8/25
******************************************************************************/

/// Save symbols to JSON file
pub async fn save_symbols_to_file(
    symbols: &[crate::prelude::SymbolEntry],
    filename: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(symbols)?;
    tokio::fs::write(filename, json).await?;
    tracing::info!("💾 Symbols saved to {}", filename);
    Ok(())
}
