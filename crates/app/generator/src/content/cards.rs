use systemprompt_files::FilesConfig;

#[derive(Debug)]
pub struct CardData<'a> {
    pub title: &'a str,
    pub slug: &'a str,
    pub description: &'a str,
    pub image: Option<&'a str>,
    pub date: &'a str,
    pub url_prefix: &'a str,
}

pub fn normalize_image_url(image: Option<&str>) -> Option<String> {
    let img = image?;
    if img.is_empty() {
        return None;
    }

    if let Some(local_path) = convert_external_url_to_local(img) {
        return Some(local_path);
    }

    if let Some(local_path) = convert_root_images_to_content_path(img) {
        return Some(local_path);
    }

    Some(img.to_string())
}

fn convert_external_url_to_local(_url: &str) -> Option<String> {
    None
}

fn convert_root_images_to_content_path(path: &str) -> Option<String> {
    let files_config = FilesConfig::from_profile()
        .map_err(|e| {
            tracing::warn!(error = %e, "Failed to get files config for path conversion");
            e
        })
        .ok()?;
    let url_prefix = files_config.url_prefix();

    let rest = path.strip_prefix("/images/")?;

    if let Some(relative) = rest.strip_prefix("generated_images/") {
        return Some(format!("{url_prefix}/images/generated/{relative}"));
    }

    if let Some(slash_pos) = rest.find('/') {
        let source = &rest[..slash_pos];
        let relative = &rest[slash_pos + 1..];
        return Some(format!("{url_prefix}/images/{source}/{relative}"));
    }

    Some(format!("{url_prefix}/images/{rest}"))
}

pub fn get_absolute_image_url(image: Option<&str>, base_url: &str) -> Option<String> {
    let normalized = normalize_image_url(image)?;
    if normalized.starts_with("http") {
        Some(normalized)
    } else {
        Some(format!("{}{}", base_url.trim_end_matches('/'), normalized))
    }
}

pub fn generate_image_html(image: Option<&str>, alt: &str) -> String {
    normalize_image_url(image).map_or_else(
        || {
            r#"<div class="card-image card-image--placeholder">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
      <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
      <circle cx="8.5" cy="8.5" r="1.5"/>
      <polyline points="21 15 16 10 5 21"/>
    </svg>
  </div>"#
                .to_string()
        },
        |img| {
            format!(
                r#"<div class="card-image">
    <img src="{}" alt="{}" loading="lazy" />
  </div>"#,
                img, alt
            )
        },
    )
}

pub fn generate_content_card(data: &CardData) -> String {
    let image_html = generate_image_html(data.image, data.title);

    format!(
        r#"<a href="{}/{}" class="content-card-link">
  <article class="content-card">
    {}
    <div class="card-content">
      <h2 class="card-title">{}</h2>
      <p class="card-excerpt">{}</p>
      <div class="card-meta">
        <time class="card-date">{}</time>
      </div>
    </div>
  </article>
</a>"#,
        data.url_prefix, data.slug, image_html, data.title, data.description, data.date
    )
}

pub fn generate_related_card(data: &CardData, href: &str) -> String {
    let image_html = generate_image_html(data.image, data.title);
    let excerpt_lines: String = data
        .description
        .lines()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");

    format!(
        r#"<a href="{}" class="related-card-link">
  <article class="related-card">
    {}
    <div class="card-content">
      <h4 class="card-title">{}</h4>
      <p class="card-excerpt">{}</p>
      <div class="card-meta">
        <time class="card-date">{}</time>
      </div>
    </div>
  </article>
</a>"#,
        href, image_html, data.title, excerpt_lines, data.date
    )
}
