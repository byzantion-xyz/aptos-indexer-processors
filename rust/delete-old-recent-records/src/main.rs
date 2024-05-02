use tokio_postgres::{NoTls};
use std::env;

#[tokio::main]
async fn main() -> Result<(), tokio_postgres::Error> {
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL environment variable not set");
    let period = env::var("PERIOD").unwrap_or_else(|_| "30".to_string());

    let (client, connection) = tokio_postgres::connect(&database_url, NoTls).await.expect("Could not connect to database");

    println!("Removing outdated recent records (older than {} days)", period);

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("DB connection error: {}", e);
        }
    });

    for table in ["events", "transactions", "user_transactions", "signatures",
        "current_collection_datas", "current_collections_v2", "current_token_datas", "current_token_datas_v2",
        "current_token_ownerships", "current_token_ownerships_v2", "current_token_v2_metadata",
        "move_resources"
    ] {
        println!("table: {}", table);
        client.simple_query(&format!("DELETE FROM {} WHERE inserted_at < now() - interval '{} days'", table, period)).await?;
    }

    println!("Done");
    Ok(())
}
