use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AcceptedMediaType {
    #[default]
    Json,
    Markdown,
    Html,
}

impl AcceptedMediaType {
    pub const fn content_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Html => "text/html; charset=utf-8",
        }
    }

    pub const fn is_markdown(&self) -> bool {
        matches!(self, Self::Markdown)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AcceptedFormat(pub AcceptedMediaType);

impl Default for AcceptedFormat {
    fn default() -> Self {
        Self(AcceptedMediaType::Json)
    }
}

impl AcceptedFormat {
    pub const fn media_type(&self) -> AcceptedMediaType {
        self.0
    }

    pub const fn is_markdown(&self) -> bool {
        self.0.is_markdown()
    }
}

struct MediaTypeEntry {
    media_type: AcceptedMediaType,
    quality: f32,
}

fn parse_accept_header(header_value: &str) -> AcceptedFormat {
    let mut entries = Vec::new();

    for part in header_value.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let (media_type_str, params) = part
            .split_once(';')
            .map_or((part, ""), |(m, p)| (m.trim(), p));

        let quality = params
            .split(';')
            .find_map(|p| {
                let p = p.trim();
                if let Some(q_str) = p.strip_prefix("q=") {
                    q_str.parse::<f32>().ok().map(|q| q.clamp(0.0, 1.0))
                } else {
                    None
                }
            })
            .unwrap_or(1.0);

        let media_type = match media_type_str.to_lowercase().as_str() {
            "text/markdown" | "text/x-markdown" => Some(AcceptedMediaType::Markdown),
            "application/json" | "*/*" => Some(AcceptedMediaType::Json),
            "text/html" | "application/xhtml+xml" => Some(AcceptedMediaType::Html),
            _ => None,
        };

        if let Some(mt) = media_type {
            entries.push(MediaTypeEntry {
                media_type: mt,
                quality,
            });
        }
    }

    entries.sort_by(|a, b| {
        b.quality
            .partial_cmp(&a.quality)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let media_type = entries
        .first()
        .map_or(AcceptedMediaType::Json, |e| e.media_type);

    AcceptedFormat(media_type)
}

pub async fn content_negotiation_middleware(mut request: Request, next: Next) -> Response {
    let accepted_format = request
        .headers()
        .get(http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map_or_else(AcceptedFormat::default, parse_accept_header);

    request.extensions_mut().insert(accepted_format);

    next.run(request).await
}
