use actix_web::{web, App, HttpServer};
use clap::Parser;
use smotrim_rss_proxy::{proxy, AppState, Args};
use tokio::sync::Mutex;

mod database;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let address = format!("{}:{}", args.ip, args.port);

    let conn = database::init_db(&args.db_path).await.map_err(|e| {
        eprintln!("Failed to initialize database: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, "Database initialization failed")
    })?;
    let db_conn = Mutex::new(conn.clone());

    let app_state = web::Data::new(AppState {
        config: args,
        db: db_conn,
    });

    println!("Server running at http://{}", address);
    let res = HttpServer::new(move || App::new().app_data(app_state.clone()).service(proxy))
        .bind(&address)?
        .run()
        .await;

    let _ = conn.close().await;
    res
}
