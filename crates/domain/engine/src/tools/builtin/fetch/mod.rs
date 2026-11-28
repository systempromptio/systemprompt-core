//! The fetch tool: GET a URL and return it as Markdown, plain text or raw HTML;
//! images come back as attachments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod download;

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use self::download::{Fetched, Reply, Request, download};
use crate::tools::builtin::html::{Flavor, convert};
use crate::tools::config::FetchConfig;
use crate::tools::contract::{Attachment, PermissionRequest, Tool, ToolContext, ToolOutput};
use crate::tools::error::ToolError;
use crate::tools::repair::parse_input;
use crate::tools::truncate::{Keep, OutputLimits, truncate_output};

/// Fetches web pages.
#[derive(Debug, Clone)]
pub struct FetchTool {
    config: FetchConfig,
    output: OutputLimits,
    client: Option<reqwest::Client>,
}

impl FetchTool {
    pub fn new(config: FetchConfig, output: OutputLimits) -> Self {
        let built = reqwest::Client::builder()
            .user_agent(config.user_agent.clone())
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .build();
        if let Err(err) = &built {
            tracing::warn!(error = %err, "fetch tool has no HTTP client");
        }
        Self {
            config,
            output,
            client: built.ok(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    #[default]
    Markdown,
    Text,
    Html,
}

impl Format {
    const fn accept(self) -> &'static str {
        match self {
            Self::Markdown => "text/markdown;q=1.0, text/html;q=0.9, text/plain;q=0.8, */*;q=0.5",
            Self::Text => "text/plain;q=1.0, text/html;q=0.9, */*;q=0.5",
            Self::Html => "text/html;q=1.0, */*;q=0.5",
        }
    }
}

#[derive(Debug, Deserialize)]
struct Input {
    url: String,
    #[serde(default)]
    format: Format,
    timeout: Option<u64>,
}

#[async_trait]
impl Tool for FetchTool {
    fn name(&self) -> &str {
        crate::tools::names::FETCH
    }

    fn description(&self) -> String {
        format!(
            "Download a web page or file over HTTP(S) with a GET request and return its content. HTML is converted \
             to Markdown by default (`format`: `markdown`, `text` or `html` for the raw page); JSON and other text \
             come back as they are; images come back as attachments. Responses over {} bytes are refused and long \
             text is truncated with the rest saved to a file. The default timeout is {} seconds (`timeout`, up to {}). \
             Use it for documentation, issues or API references the user points to; it cannot log in, run scripts or \
             fill in forms.",
            self.config.max_bytes,
            self.config.default_timeout.as_secs(),
            self.config.max_timeout.as_secs()
        )
    }

    // JSON: the tool's input schema, sent to the model as JSON Schema.
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "The http:// or https:// URL to fetch."},
                "format": {"type": "string", "enum": ["markdown", "text", "html"], "description": "How to return HTML pages. Defaults to markdown."},
                "timeout": {"type": "integer", "minimum": 1, "description": "Timeout in seconds."}
            },
            "required": ["url"]
        })
    }

    // JSON: the object the model wrote, checked against the schema above.
    async fn run(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput, ToolError> {
        let input: Input = parse_input(&self.schema(), input)?;
        let url =
            Url::parse(input.url.trim()).map_err(|err| ToolError::malformed("invalid URL", err))?;
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ToolError::failed("the HTTP client could not be initialised"))?;
        let timeout = input
            .timeout
            .map_or(self.config.default_timeout, Duration::from_secs)
            .min(self.config.max_timeout);
        let mut url = url;
        let mut approved: Option<url::Origin> = None;
        let mut hops = 0;
        let fetched = loop {
            let host = permitted_host(&url)?;
            if approved.as_ref() != Some(&url.origin()) {
                ask_host(ctx, &url, host).await?;
                approved = Some(url.origin());
            }
            let request = Request {
                url: &url,
                accept: input.format.accept(),
                timeout,
                max_bytes: self.config.max_bytes,
            };
            let reply = tokio::select! {
                result = download(client, request) => result?,
                () = ctx.cancel.cancelled() => return Err(ToolError::Aborted),
            };
            match reply {
                Reply::Page(fetched) => break fetched,
                Reply::Redirect(next) if hops < MAX_REDIRECTS => {
                    hops += 1;
                    url = next;
                },
                Reply::Redirect(_) => {
                    return Err(ToolError::invalid(format!(
                        "{url} redirects more than {MAX_REDIRECTS} times"
                    )));
                },
            }
        };
        let mime = fetched
            .content_type
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if mime.starts_with("image/") && mime != "image/svg+xml" {
            return Ok(image_output(&url, &mime, &fetched));
        }
        self.text_output(fetched, &mime, input.format, ctx).await
    }
}

impl FetchTool {
    async fn text_output(
        &self,
        fetched: Fetched,
        mime: &str,
        format: Format,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let texty = mime.is_empty()
            || mime.starts_with("text/")
            || mime.contains("json")
            || mime.contains("xml")
            || mime.contains("javascript");
        let text = match String::from_utf8(fetched.body) {
            Ok(text) => text,
            Err(err) if texty => String::from_utf8_lossy(err.as_bytes()).into_owned(),
            Err(_) => {
                return Err(ToolError::invalid(format!(
                    "{} returned {mime}, which is not text",
                    fetched.final_url
                )));
            },
        };
        let is_html =
            mime.contains("html") || (mime.is_empty() && text.trim_start().starts_with('<'));
        let rendered = match (format, is_html) {
            (Format::Markdown, true) => convert(&text, Flavor::Markdown),
            (Format::Text, true) => convert(&text, Flavor::Text),
            _ => text,
        };
        let size = rendered.len();
        let capped = truncate_output(
            rendered,
            self.output,
            Keep::Head,
            &ctx.data_dir,
            ctx.call_id.as_str(),
        )
        .await;
        Ok(
            ToolOutput::new(fetched.final_url.clone(), capped.text).with_metadata(json!({
                "url": fetched.final_url,
                "mime": mime,
                "bytes": size,
                "truncated": capped.truncated,
            })),
        )
    }
}

const MAX_REDIRECTS: usize = 10;

fn permitted_host(url: &Url) -> Result<String, ToolError> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(ToolError::invalid(
            "only http:// and https:// URLs can be fetched",
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ToolError::invalid(
            "URLs with embedded credentials cannot be fetched",
        ));
    }
    url.host_str()
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| ToolError::invalid("the URL has no host"))
}

async fn ask_host(ctx: &ToolContext, url: &Url, host: String) -> Result<(), ToolError> {
    ctx.ask(PermissionRequest {
        permission: "fetch".to_owned(),
        patterns: vec![host.clone()],
        always: vec![host.clone()],
        title: format!("Fetch {url}"),
        metadata: json!({ "url": url.as_str(), "host": host }),
    })
    .await
}

fn image_output(url: &Url, mime: &str, fetched: &Fetched) -> ToolOutput {
    let filename = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    let attachment = Attachment {
        mime: mime.to_owned(),
        filename,
        data: base64::engine::general_purpose::STANDARD.encode(&fetched.body),
    };
    let text = format!(
        "Attached {mime} from {} ({} bytes).",
        fetched.final_url,
        fetched.body.len()
    );
    ToolOutput::new(fetched.final_url.clone(), text)
        .with_metadata(json!({
            "url": fetched.final_url,
            "mime": mime,
            "bytes": fetched.body.len(),
        }))
        .with_attachment(attachment)
}
