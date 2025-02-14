use tokio_rusqlite::{Connection, Result};

pub async fn init_db(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path).await.unwrap();
    conn.call(|conn| {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS items (
                id INTEGER PRIMARY KEY,
                brand_id INTEGER,
                title TEXT,
                description TEXT,
                size INTEGER,
                duration TEXT,
                published TEXT,
                image TEXT
                )",
            [],
        )?;
        Ok(())
    })
    .await?;

    Ok(conn)
}
