use actix_web::web;
use chrono::Utc;
use reqwest::header::{HeaderMap, USER_AGENT};
use scraper::{Html, Selector};
use serde_json::Value;
use std::error::Error;
use tokio_rusqlite::params;

use crate::custom_date::format_rfc822;
use crate::custom_date::parse_custom_date;
use crate::AppState;

fn build_user_agent() -> HeaderMap {
    let custom_user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, custom_user_agent.parse().unwrap());
    headers
}

pub async fn fetch_api_response(url: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let response_text = client
        .get(url)
        .headers(build_user_agent())
        .send()
        .await?
        .text()
        .await?;

    Ok(response_text)
}

pub async fn fetch_brand_description(id: u64) -> Result<String, Box<dyn Error>> {
    let url = format!("https://smotrim.ru/brand/{}", id);
    let response = reqwest::get(url).await?.text().await?;
    let document = Html::parse_document(&response);

    let selector = Selector::parse("div.brand-main-item__body").unwrap();

    if let Some(content) = document.select(&selector).next() {
        // Извлекаем текст из элемента и очищаем от HTML-тегов
        let text = content.text().collect::<Vec<_>>().join(" ");
        let text = text.replace('\n', " ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");

        return Ok(text);
    }

    Err("Can't parse brand description".into())
}

pub async fn get_content_length(url: &str) -> Result<u64, Box<dyn Error>> {
    let client = reqwest::Client::new();
    let head = client.head(url).headers(build_user_agent()).send().await?;

    match head.headers().get(reqwest::header::CONTENT_LENGTH) {
        Some(content_length) => {
            let size = content_length.to_str()?.parse::<u64>()?;
            Ok(size)
        }
        _ => Err("Can't get audio size".into()),
    }
}

#[derive(Debug, Clone)]
pub struct Podcast {
    id: u64,
    title: String,
    description: String,
    link: String,
    image: String,
    episodes: Vec<Episode>,
}

impl Podcast {
    pub async fn from_json(
        app_data: web::Data<AppState>,
        id: u64,
        json: &Value,
    ) -> Result<Self, Box<dyn Error>> {
        let episodes = create_episodes(app_data, json, id).await?;
        let title = json["contents"][0]["list"][0]["title"].to_string();
        let link = format!("https://smotrim.ru/brand/{}", id);
        let description = fetch_brand_description(id).await.unwrap_or("".into());
        let image =
            json["contents"][0]["list"][0]["player"]["preview"]["source"]["main"].to_string();

        Ok(Self {
            id,
            title,
            link,
            description,
            image,
            episodes,
        })
    }

    pub fn to_string(&self) -> String {
        let build_date = format_rfc822(Utc::now());
        let mut episodes = String::new();
        for item in self.episodes.iter() {
            let item_rss = item.to_string();
            episodes.push_str(&item_rss.to_string());
        }

        let app_name = env!("CARGO_PKG_NAME");
        let app_version = env!("CARGO_PKG_VERSION");
        let funding_url = "https://pay.cloudtips.ru/p/a368e9f8";

        format!(
            r#"<?xml version='1.0' encoding='UTF-8'?>
<rss xmlns:atom="http://www.w3.org/2005/Atom" xmlns:content="http://purl.org/rss/1.0/modules/content/" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:podcast="https://podcastindex.org/namespace/1.0" version="2.0">
<channel>
    <title>{title}</title>
    <link>{link}</link>
    <description>{description}</description>
    <lastBuildDate>{build_date}</lastBuildDate>
    <itunes:explicit>yes</itunes:explicit>
    <itunes:image href={image}/>
    <itunes:owner>
        <itunes:name>Sergey</itunes:name>
        <itunes:email>me@coyotle.ru</itunes:email>
    </itunes:owner>
    <language>ru-RU</language>
    <generator>{app_name} v{app_version}</generator>
    <docs>http://www.rssboard.org/rss-specification</docs>
    <podcast:funding url="{funding_url}">Поддержите работу проекта</podcast:funding>
    {episodes}
</channel>
</rss>"#,
            title = self.title,
            link = self.link,
            description = self.description,
            build_date = build_date,
            image = self.image,
            funding_url = funding_url,
            episodes = episodes,
        )
    }
}

async fn create_episodes(
    app_data: web::Data<AppState>,
    json: &Value,
    brand_id: u64,
) -> Result<Vec<Episode>, Box<dyn Error>> {
    let conn = app_data.db.lock().await;

    let items: Vec<Value> = serde_json::from_value(json["contents"][0]["list"].clone())?;

    let mut result = vec![];
    for item in items {
        let item_id = item["id"].to_string();
        if item["isActive"].to_string() == "true" {
            continue;
        }

        let media_url = format!(
            "https://vgtrk-podcast.cdnvideo.ru/audio/listen?id={}",
            item_id
        );

        let item_id_clone = item_id.to_string();
        let db_media_size = conn
            .call(move |conn| {
                let mut stmt = conn.prepare("SELECT size FROM items WHERE id = ?")?;
                let size = stmt.query_row(params![item_id_clone], |row| row.get(0));
                Ok(size)
            })
            .await?;

        let media_size = match db_media_size {
            Ok(Some(size)) => size,
            _ => match get_content_length(&media_url).await {
                Ok(size) => size,
                Err(err) => {
                    eprintln!("ERROR: skip episode {}. {}", item["id"], err);
                    continue;
                }
            },
        };

        let episode = Episode::from_json(&item, brand_id, media_size)?;

        let ep = episode.clone();
        result.push(episode);

        if let Err(_) = db_media_size {
            let _ = conn
                    .call(move |conn| {
                        let mut stmt = conn.prepare(
                    "INSERT INTO items (id, brand_id, title, description, size, duration, published, image) 
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?);",
                )?;
                        let rows_affected = stmt.execute(params![
                            ep.id,
                            ep.brand_id,
                            ep.title,
                            ep.description,
                            ep.media_size,
                            ep.duration,
                            ep.published,
                            ep.image
                        ]);
                        Ok(rows_affected)
                    })
                    .await?;
        }
    }

    Ok(result)
}

//
#[derive(Debug, Clone)]
struct Episode {
    id: String,
    brand_id: String,
    title: String,
    description: String,
    duration: String,
    published: String,
    image: String,
    media_url: String,
    media_size: u64,
}

impl Episode {
    fn from_json(item: &Value, brand_id: u64, media_size: u64) -> Result<Self, Box<dyn Error>> {
        let id = item["id"].to_string();
        let title = item["anons"].to_string().replace("\\\"", "");
        let description = item["description"].to_string().replace("\\\"", "");
        let duration = item["duration"].to_string().replace("\"", "");
        let published = format_rfc822(parse_custom_date(&item["published"].to_string())?);
        let image = item["player"]["preview"]["source"]["main"]
            .to_string()
            .trim_matches('"')
            .to_string();
        let media_url = format!("https://vgtrk-podcast.cdnvideo.ru/audio/listen?id={}", id);

        Ok(Episode {
            id,
            brand_id: brand_id.to_string(),
            title,
            description,
            duration,
            published,
            image,
            media_url,
            media_size,
        })
    }

    fn to_string(&self) -> String {
        format!(
            r#"<item>
            <title>{title}</title>
            <description>{description}</description>
            <guid isPermaLink="true">{media_url}</guid>
            <enclosure url="{media_url}" length="{media_size}" type="audio/mpeg"/>
            <itunes:duration>{duration}</itunes:duration>
            <pubDate>{published}</pubDate>
            <itunes:image href="{image}"/>
        </item>
        "#,
            title = self.title,
            description = self.description,
            media_url = self.media_url,
            media_size = self.media_size,
            duration = self.duration,
            published = self.published,
            image = self.image
        )
    }
}
