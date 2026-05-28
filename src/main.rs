use std::{
    ops::AddAssign,
    sync::{Arc, LazyLock, Mutex},
};

use actix_web::{App, HttpResponse, HttpServer, Responder, get, http, post, web};
use actix_web_validator::Query;
use rand::distr::{Alphanumeric, SampleString};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use validator::Validate;

static RE_HTTP_SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?://.+").unwrap());

const ID_SIZE: u8 = 5;

#[derive(Debug, Clone)]
struct ShortenedUrl {
    pub id: String,
    pub full_url: String,
    pub visits: Arc<Mutex<usize>>,
}

impl Display for ShortenedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.id, self.full_url)
    }
}

impl ShortenedUrl {
    pub fn new(url: impl Into<String>) -> Self {
        let mut rng = rand::rng();
        let id = Alphanumeric.sample_string(&mut rng, ID_SIZE as usize);
        Self {
            id,
            full_url: url.into(),
            visits: Arc::new(Mutex::new(0)),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let urls = vec![
        ShortenedUrl::new("https://google.com"),
        ShortenedUrl::new("https://youtube.com"),
        ShortenedUrl::new("https://www.wikipedia.com"),
        ShortenedUrl::new("https://dorianmoy.fr"),
    ];
    let data = web::Data::new(Mutex::new(urls.clone()));
    println!("List of shortened url:");
    for url in urls {
        println!("  /{} -> {}", url.id, url.full_url);
    }
    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .service(shorten_url)
            .service(redirect_to_url_for_id)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}

#[derive(Debug, Deserialize, Serialize, Clone, Validate)]
pub struct ShortenUrlDto {
    #[validate(url, regex(path = *RE_HTTP_SCHEME))]
    pub(crate) url: String,
}

#[post("/shorten")]
async fn shorten_url(
    query: Query<ShortenUrlDto>,
    urls: web::Data<Mutex<Vec<ShortenedUrl>>>,
) -> impl Responder {
    let url = query.into_inner().url;
    let id = {
        let mut urls = urls.lock().expect("Lock poisoned");
        if let Some(shortened_url) = urls
            .iter()
            .find(|shortened_url| url == shortened_url.full_url)
        {
            shortened_url.id.clone()
        } else {
            let new_shortened_url = ShortenedUrl::new(url);
            let id = new_shortened_url.id.clone();

            urls.push(new_shortened_url);
            id
        }
    };
    HttpResponse::Ok().body(id)
}

#[get("/{id}")]
async fn redirect_to_url_for_id(
    id: web::Path<String>,
    urls: web::Data<Mutex<Vec<ShortenedUrl>>>,
) -> impl Responder {
    let id = id.into_inner();
    let shortened_url = {
        let urls = urls.lock().expect("Lock poisoned");
        urls.iter().find(|url| url.id == id).cloned()
    };

    if let Some(shortened_url) = shortened_url {
        shortened_url
            .visits
            .lock()
            .expect("Could not write visits")
            .add_assign(1);
        HttpResponse::TemporaryRedirect()
            .insert_header((http::header::LOCATION, shortened_url.full_url.clone()))
            .body(format!("Redirecting to {}...", shortened_url.full_url))
    } else {
        HttpResponse::NotFound().body("No url for this id")
    }
}
