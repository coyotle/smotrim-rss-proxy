use actix_web::{get, web, HttpResponse, Responder};
use chrono::Utc;
use clap::Parser;
use smotrim::Podcast;
use tokio::sync::Mutex;
use tokio_rusqlite::Connection;

mod cache;
mod database;
mod smotrim;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Args {
    /// IP для запуска сервера
    #[clap(short, long, default_value = "127.0.0.1")]
    pub ip: String,

    /// TCP порт сервера
    #[clap(short, long, default_value = "3000")]
    pub port: u16,

    /// Количество эпизодов
    #[clap(short, long, default_value = "20")]
    pub limit: u16,

    /// Время жизни кэша в секундах
    #[clap(short, long, default_value = "600")]
    pub cache_lifetime: u16,

    /// Путь к sqlite базе для хранения данных
    #[clap(short, long, default_value = "data.sqlite")]
    pub db_path: String,
}

pub struct AppState {
    pub config: Args,
    pub db: Mutex<Connection>,
}

#[get("/brand/{id}")]
async fn proxy(id: web::Path<String>, app_data: web::Data<AppState>) -> impl Responder {
    let brand_id: u64 = match id.parse() {
        Ok(num) => num,
        Err(err) => {
            eprintln!("Error parse brand_id: {}", err);
            return HttpResponse::BadRequest().body("Failed to parse brand_id");
        }
    };

    let limit = app_data.config.limit;
    let api_url = format!("https://smotrim.ru/api/audios?brandId={brand_id}&limit={limit}");

    let mut feed_cache = cache::FEEDS_CACHE.lock().await;

    let cached_data = feed_cache.get(&id.to_string());
    let current_time = Utc::now().timestamp();

    if let Some(cached_rss) = cached_data {
        let cache_lifetime = app_data.config.cache_lifetime;
        if current_time - cached_rss.cached_at < cache_lifetime.into() {
            return HttpResponse::Ok()
                .content_type("application/rss+xml")
                .body(cached_rss.body.clone());
        }
    }

    let json_text_result = smotrim::fetch_text(&api_url).await;
    let json_text = match json_text_result {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error fetching text: {}", e);
            return HttpResponse::BadGateway().body("Failed to fetch data");
        }
    };

    let json_result = serde_json::from_str(&json_text);
    let json = match json_result {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            return HttpResponse::InternalServerError().body("Failed to parse upstream response");
        }
    };

    let podcast_result = Podcast::from_json(app_data, brand_id, &json).await;

    let podcast = match podcast_result {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to create Podcast from JSON: {}", e);
            return HttpResponse::InternalServerError().body("Failed to build XML");
        }
    };

    let rss = podcast.to_string();

    feed_cache.insert(
        id.to_string(),
        cache::RssCache {
            body: rss.clone(),
            cached_at: current_time,
        },
    );

    HttpResponse::Ok()
        .content_type("application/rss+xml")
        .body(rss)
}
