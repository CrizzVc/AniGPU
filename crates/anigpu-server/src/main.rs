use anigpu_core::{
    providers,
    sources::{self, Source},
};
use axum::{
    extract::Query,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Json},
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

#[derive(Deserialize)]
struct SourceQuery {
    source: Option<String>,
}

#[derive(Deserialize)]
struct UrlQuery {
    url: String,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Deserialize)]
struct BrowseQuery {
    page: Option<u32>,
}

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    #[serde(flatten)]
    data: T,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct DataWrapper<T> {
    data: T,
}

#[derive(Serialize)]
struct ServersWrapper<T> {
    servers: T,
}

// Utility to get the source based on query parameter or header
fn get_source(query: &Option<String>, headers: &HeaderMap) -> &'static dyn Source {
    let source_id = query.as_deref().or_else(|| {
        headers
            .get("x-source")
            .and_then(|h| h.to_str().ok())
    });
    sources::get_source(source_id)
}

async fn proxy_image(
    Query(url_query): Query<UrlQuery>,
) -> Response {
    let url = url_query.url;
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    let mut request = client.get(&url);
    request = request
        .header("Referer", "https://animeav1.com/")
        .header("Accept", "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8")
        .header("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
        .header("Connection", "keep-alive");

    match request.send().await {
        Ok(res) => {
            let status = res.status();
            let content_type = res
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/jpeg")
                .to_string();

            let bytes = match res.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::error!("Failed to read image bytes: {}", e);
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            };

            Response::builder()
                .status(status)
                .header("content-type", content_type)
                .body(axum::body::Body::from(bytes))
                .unwrap()
        }
        Err(e) => {
            tracing::error!("Proxy image error: {}", e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

// Helper for error responses
fn internal_error(e: anigpu_core::Error, msg: &str) -> (StatusCode, Json<ErrorResponse>) {

    tracing::error!("{}: {}", msg, e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}


// Route Handlers

async fn get_latest(
    headers: HeaderMap,
    Query(query): Query<SourceQuery>,
) -> std::result::Result<Json<ApiResponse<DataWrapper<Vec<anigpu_core::models::LatestItem>>>>, (StatusCode, Json<ErrorResponse>)> {
    let source = get_source(&query.source, &headers);
    match source.get_latest().await {
        Ok(data) => Ok(Json(ApiResponse {
            success: true,
            data: DataWrapper { data },
        })),
        Err(e) => Err(internal_error(e, "Failed to fetch latest episodes.")),
    }
}

async fn get_anime_details(
    headers: HeaderMap,
    Query(query): Query<SourceQuery>,
    Query(url_query): Query<UrlQuery>,
) -> std::result::Result<Json<ApiResponse<DataWrapper<anigpu_core::models::AnimeDetails>>>, (StatusCode, Json<ErrorResponse>)> {
    let source = get_source(&query.source, &headers);
    match source.get_details(&url_query.url).await {
        Ok(data) => Ok(Json(ApiResponse {
            success: true,
            data: DataWrapper { data },
        })),
        Err(e) => Err(internal_error(e, "Failed to fetch anime details.")),
    }
}

async fn get_servers(
    headers: HeaderMap,
    Query(query): Query<SourceQuery>,
    Query(url_query): Query<UrlQuery>,
) -> std::result::Result<Json<ApiResponse<ServersWrapper<Vec<anigpu_core::models::ServerItem>>>>, (StatusCode, Json<ErrorResponse>)> {
    let source = get_source(&query.source, &headers);
    match source.get_servers(&url_query.url).await {
        Ok(servers) => Ok(Json(ApiResponse {
            success: true,
            data: ServersWrapper { servers },
        })),
        Err(e) => Err(internal_error(e, "Failed to fetch servers.")),
    }
}

async fn search(
    headers: HeaderMap,
    Query(query): Query<SourceQuery>,
    Query(search_query): Query<SearchQuery>,
) -> std::result::Result<Json<ApiResponse<DataWrapper<Vec<anigpu_core::models::CardItem>>>>, (StatusCode, Json<ErrorResponse>)> {
    let source = get_source(&query.source, &headers);
    match source.search(&search_query.q).await {
        Ok(data) => Ok(Json(ApiResponse {
            success: true,
            data: DataWrapper { data },
        })),
        Err(e) => Err(internal_error(e, "Failed to search anime.")),
    }
}

async fn browse(
    headers: HeaderMap,
    Query(query): Query<SourceQuery>,
    Query(browse_query): Query<BrowseQuery>,
) -> std::result::Result<Json<ApiResponse<DataWrapper<Vec<anigpu_core::models::CardItem>>>>, (StatusCode, Json<ErrorResponse>)> {
    let source = get_source(&query.source, &headers);
    let page = browse_query.page.unwrap_or(1);
    match source.browse(page).await {
        Ok(data) => Ok(Json(ApiResponse {
            success: true,
            data: DataWrapper { data },
        })),
        Err(e) => Err(internal_error(e, "Failed to browse anime.")),
    }
}

async fn extract(
    Query(url_query): Query<UrlQuery>,
) -> std::result::Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match providers::extract(&url_query.url).await {
        Ok(result) => {
            // we want `{ success: true, ...result }`
            let mut val = serde_json::to_value(&result).unwrap();
            if let Some(obj) = val.as_object_mut() {
                obj.insert("success".to_string(), serde_json::Value::Bool(true));
            }
            Ok(Json(val))
        }
        Err(e) => Err(internal_error(e, "Extraction failed")),
    }
}

async fn list_sources() -> Json<ApiResponse<DataWrapper<Vec<anigpu_core::models::SourceInfo>>>> {
    Json(ApiResponse {
        success: true,
        data: DataWrapper {
            data: sources::all_sources(),
        },
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let app = Router::new()
        .route("/api/latest", get(get_latest))
        .route("/api/anime-details", get(get_anime_details))
        .route("/api/servers", get(get_servers))
        .route("/api/search", get(search))
        .route("/api/browse", get(browse))
        .route("/api/extract", get(extract))
        .route("/api/sources", get(list_sources))
        .route("/api/image", get(proxy_image))
        .layer(CorsLayer::permissive());


    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port).parse::<SocketAddr>().unwrap();
    
    tracing::info!("Modular Anime Backend running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
