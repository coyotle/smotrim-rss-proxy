use actix_web::{
    http::{header, Method},
    route, web, HttpRequest, HttpResponse, Responder,
};
use chrono::Utc;
use clap::Parser;
use std::time::SystemTime;
use tokio::sync::Mutex;
use tokio_rusqlite::Connection;

pub mod cache;
pub mod custom_date;
pub mod database;
pub mod smotrim;

use smotrim::fetch_api_response;
use smotrim::Podcast;

#[derive(Parser, Debug)]
#[clap(about, version, author)]
pub struct Args {
    #[clap(short, long, default_value = "127.0.0.1")]
    pub ip: String,

    #[clap(short, long, default_value = "3000")]
    pub port: u16,

    #[clap(short, long, default_value = "20")]
    pub limit: u16,

    #[clap(short, long, default_value = "600")]
    pub cache_lifetime: u16,

    #[clap(short, long, default_value = "data.sqlite")]
    pub db_path: String,
}

pub struct AppState {
    pub config: Args,
    pub db: Mutex<Connection>,
}

#[route("/brand/{id}", method = "GET", method = "HEAD")]
async fn proxy(
    id: web::Path<String>,
    app_data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    let brand_id: u64 = match id.parse() {
        Ok(num) => num,
        Err(err) => {
            eprintln!("Error parsing brand_id: {}", err);
            return HttpResponse::BadRequest().body("Failed to parse brand_id");
        }
    };

    let limit = app_data.config.limit;
    let api_url = format!("https://smotrim.ru/api/audios?brandId={brand_id}&limit={limit}");

    let mut feed_cache = cache::FEEDS_CACHE.lock().await;

    if let Some(cached_rss) = feed_cache.get(&id.to_string()) {
        let cache_lifetime = app_data.config.cache_lifetime;
        if Utc::now().timestamp() - cached_rss.cached_at < cache_lifetime.into() {
            return create_response(&req, &cached_rss.body, cached_rss.cached_at);
        }
    }

    let json_text = match fetch_api_response(&api_url).await {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Error fetching api response: {}", e);
            return HttpResponse::BadGateway().body("Failed to fetch api response");
        }
    };

    let json = match serde_json::from_str(&json_text) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("Failed to parse JSON: {}", e);
            return HttpResponse::InternalServerError().body("Failed to parse upstream response");
        }
    };

    let podcast = match Podcast::from_json(app_data, brand_id, &json).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to create Podcast from JSON: {}", e);
            return HttpResponse::InternalServerError().body("Failed to build XML");
        }
    };

    let rss = podcast.to_string();
    let current_time = Utc::now().timestamp();

    feed_cache.insert(
        id.to_string(),
        cache::RssCache {
            body: rss.clone(),
            cached_at: current_time,
        },
    );

    create_response(&req, &rss, current_time)
}

fn create_response(req: &HttpRequest, body: &str, cached_at: i64) -> HttpResponse {
    let last_modified = header::HttpDate::from(
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(cached_at as u64),
    );

    let mut response = HttpResponse::Ok();
    response.insert_header((header::CONTENT_TYPE, "application/rss+xml"));
    response.insert_header((header::LAST_MODIFIED, last_modified.to_string()));

    if req.method() == Method::HEAD {
        response
            .insert_header((header::CONTENT_LENGTH, body.len()))
            .finish()
    } else {
        response.body(body.to_string())
    }
}
