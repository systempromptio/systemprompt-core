# Content CLI Domain Plan

## Overview

Add comprehensive content management CLI commands exposing the existing `IngestionService`, `SearchService`, `ContentService`, `LinkGenerationService`, and `LinkAnalyticsService` functionality.

## Proposed Structure

```
content
├── list [--source SOURCE_ID] [--limit N] [--offset N]
├── show <CONTENT_ID|SLUG> [--source SOURCE_ID]
├── search <QUERY> [--category CATEGORY_ID] [--limit N]
├── ingest <DIRECTORY> --source SOURCE_ID [--category CATEGORY_ID] [--recursive] [--override]
├── delete <CONTENT_ID> --yes
├── delete-source <SOURCE_ID> --yes
├── popular [--source SOURCE_ID] [--days N] [--limit N]
│
├── link
│   ├── generate --url URL [--campaign CAMPAIGN] [--utm-source SRC] [--utm-medium MED]
│   ├── show <SHORT_CODE>
│   ├── list [--campaign CAMPAIGN_ID] [--content CONTENT_ID]
│   └── performance <LINK_ID>
│
└── analytics
    ├── clicks <LINK_ID> [--limit N] [--offset N]
    ├── campaign <CAMPAIGN_ID>
    └── journey [--limit N]
```

## File Structure

```
crates/entry/cli/src/commands/content/
├── mod.rs              # ContentCommands enum + dispatch
├── types.rs            # Output types (ContentListOutput, ContentDetailOutput, etc.)
├── list.rs             # content list
├── show.rs             # content show
├── search.rs           # content search
├── ingest.rs           # content ingest
├── delete.rs           # content delete
├── delete_source.rs    # content delete-source
├── popular.rs          # content popular
├── link/
│   ├── mod.rs          # LinkCommands enum + dispatch
│   ├── generate.rs     # content link generate
│   ├── show.rs         # content link show
│   ├── list.rs         # content link list
│   └── performance.rs  # content link performance
└── analytics/
    ├── mod.rs          # AnalyticsCommands enum + dispatch
    ├── clicks.rs       # content analytics clicks
    ├── campaign.rs     # content analytics campaign
    └── journey.rs      # content analytics journey
```

## Command Details

### `content list`

List content with pagination and source filtering.

```bash
content list                            # List first 20 content items
content list --source blog              # Filter by source
content list --limit 50 --offset 100    # Paginate
content list --json                     # JSON output
```

**Args:**
```rust
#[derive(Args)]
pub struct ListArgs {
    #[arg(long, help = "Filter by source ID")]
    pub source: Option<String>,

    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ContentListOutput {
    pub items: Vec<ContentSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ContentSummary {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub kind: String,
    pub source_id: SourceId,
    pub category_id: Option<CategoryId>,
    pub published_at: Option<DateTime<Utc>>,
    pub view_count: i64,
}
```

**Service Calls:**
- `ContentRepository::list()` - All content
- `ContentRepository::list_by_source()` - Filtered by source

### `content show`

Show detailed content information.

```bash
content show content_abc123             # By ID
content show my-article --source blog   # By slug + source
```

**Args:**
```rust
#[derive(Args)]
pub struct ShowArgs {
    #[arg(help = "Content ID or slug")]
    pub identifier: String,

    #[arg(long, help = "Source ID (required when using slug)")]
    pub source: Option<String>,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ContentDetailOutput {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub body: String,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub keywords: Vec<String>,
    pub kind: String,
    pub image: Option<String>,
    pub category_id: Option<CategoryId>,
    pub source_id: SourceId,
    pub version_hash: String,
    pub is_public: bool,
    pub links: Vec<ContentLink>,
    pub updated_at: DateTime<Utc>,
}
```

**Service Calls:**
- `ContentRepository::get_by_id()` - By ID
- `ContentRepository::get_by_source_and_slug()` - By slug + source

### `content search`

Full-text search content.

```bash
content search "kubernetes"             # Search all content
content search "rust" --category tutorials --limit 10
```

**Args:**
```rust
#[derive(Args)]
pub struct SearchArgs {
    #[arg(help = "Search query")]
    pub query: String,

    #[arg(long, help = "Filter by category ID")]
    pub category: Option<String>,

    #[arg(long, default_value = "20")]
    pub limit: i64,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SearchOutput {
    pub results: Vec<SearchResultRow>,
    pub total: i64,
    pub query: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SearchResultRow {
    pub id: ContentId,
    pub slug: String,
    pub title: String,
    pub description: Option<String>,
    pub image: Option<String>,
    pub view_count: i64,
    pub source_id: SourceId,
    pub category_id: Option<CategoryId>,
}
```

**Service Call:** `SearchService::search()` or `SearchService::search_by_category()`

### `content ingest`

Ingest markdown files from a directory.

```bash
content ingest ./posts --source blog
content ingest ./docs --source docs --category tutorials --recursive
content ingest ./posts --source blog --override
```

**Args:**
```rust
#[derive(Args)]
pub struct IngestArgs {
    #[arg(help = "Directory path")]
    pub directory: PathBuf,

    #[arg(long, help = "Source ID (required)")]
    pub source: Option<String>,

    #[arg(long, help = "Category ID")]
    pub category: Option<String>,

    #[arg(long, help = "Scan recursively")]
    pub recursive: bool,

    #[arg(long, help = "Override existing content")]
    pub r#override: bool,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct IngestOutput {
    pub files_found: usize,
    pub files_processed: usize,
    pub errors: Vec<String>,
    pub success: bool,
}
```

**Service Call:** `IngestionService::ingest_directory()`

### `content delete`

Delete content by ID.

```bash
content delete content_abc123 --yes
```

**Args:**
```rust
#[derive(Args)]
pub struct DeleteArgs {
    #[arg(help = "Content ID")]
    pub content_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}
```

**Service Call:** `ContentRepository::delete()`

### `content delete-source`

Delete all content from a source.

```bash
content delete-source blog --yes
```

**Args:**
```rust
#[derive(Args)]
pub struct DeleteSourceArgs {
    #[arg(help = "Source ID")]
    pub source_id: String,

    #[arg(short = 'y', long, help = "Skip confirmation")]
    pub yes: bool,
}
```

**Service Call:** `ContentRepository::delete_by_source()`

### `content popular`

Get popular content.

```bash
content popular                         # Popular across all sources
content popular --source blog           # Popular in specific source
content popular --days 7 --limit 10     # Last 7 days, top 10
```

**Args:**
```rust
#[derive(Args)]
pub struct PopularArgs {
    #[arg(long, help = "Filter by source ID")]
    pub source: Option<String>,

    #[arg(long, default_value = "30", help = "Days to look back")]
    pub days: i64,

    #[arg(long, default_value = "10")]
    pub limit: i64,
}
```

**Service Call:** `ContentRepository::get_popular_content_ids()`

### `content link generate`

Generate a trackable campaign link.

```bash
content link generate --url https://example.com/page
content link generate --url https://example.com/page --campaign launch-2024 --utm-source twitter
content link generate --url https://example.com/page --utm-source email --utm-medium newsletter --utm-campaign weekly
```

**Args:**
```rust
#[derive(Args)]
pub struct GenerateArgs {
    #[arg(long, help = "Target URL")]
    pub url: Option<String>,

    #[arg(long, help = "Campaign ID")]
    pub campaign: Option<String>,

    #[arg(long, help = "Campaign name")]
    pub campaign_name: Option<String>,

    #[arg(long, help = "Source content ID")]
    pub content: Option<String>,

    #[arg(long, help = "UTM source")]
    pub utm_source: Option<String>,

    #[arg(long, help = "UTM medium")]
    pub utm_medium: Option<String>,

    #[arg(long, help = "UTM campaign")]
    pub utm_campaign: Option<String>,

    #[arg(long, help = "UTM term")]
    pub utm_term: Option<String>,

    #[arg(long, help = "UTM content")]
    pub utm_content: Option<String>,

    #[arg(long, value_enum, default_value = "both", help = "Link type")]
    pub link_type: LinkType,
}

#[derive(Clone, ValueEnum)]
pub enum LinkType {
    Redirect,
    Utm,
    Both,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GenerateLinkOutput {
    pub short_code: String,
    pub short_url: String,
    pub target_url: String,
    pub full_url: String,
    pub link_type: String,
    pub utm_params: Option<UtmParamsOutput>,
}
```

**Service Call:** `LinkGenerationService::generate_link()`

### `content link show`

Show link details by short code.

```bash
content link show abc123
```

**Service Call:** `LinkGenerationService::get_link_by_short_code()`

### `content link list`

List links by campaign or source content.

```bash
content link list --campaign launch-2024
content link list --content content_abc123
```

**Args:**
```rust
#[derive(Args)]
pub struct LinkListArgs {
    #[arg(long, help = "Filter by campaign ID")]
    pub campaign: Option<String>,

    #[arg(long, help = "Filter by source content ID")]
    pub content: Option<String>,
}
```

**Service Calls:**
- `LinkRepository::list_links_by_campaign()`
- `LinkRepository::list_links_by_source_content()`

### `content link performance`

Show link performance metrics.

```bash
content link performance link_abc123
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct LinkPerformanceOutput {
    pub link_id: String,
    pub click_count: i64,
    pub unique_click_count: i64,
    pub conversion_count: i64,
    pub conversion_rate: f64,
}
```

**Service Call:** `LinkAnalyticsService::get_link_performance()`

### `content analytics clicks`

Show click history for a link.

```bash
content analytics clicks link_abc123
content analytics clicks link_abc123 --limit 50 --offset 100
```

**Args:**
```rust
#[derive(Args)]
pub struct ClicksArgs {
    #[arg(help = "Link ID")]
    pub link_id: String,

    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ClicksOutput {
    pub link_id: String,
    pub clicks: Vec<ClickRow>,
    pub total: i64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ClickRow {
    pub click_id: String,
    pub session_id: SessionId,
    pub user_id: Option<UserId>,
    pub clicked_at: DateTime<Utc>,
    pub referrer_page: Option<String>,
    pub device_type: Option<String>,
    pub country: Option<String>,
    pub is_conversion: bool,
}
```

**Service Call:** `LinkAnalyticsService::get_link_clicks()`

### `content analytics campaign`

Show campaign-level analytics.

```bash
content analytics campaign launch-2024
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct CampaignAnalyticsOutput {
    pub campaign_id: String,
    pub total_clicks: i64,
    pub link_count: i64,
    pub unique_visitors: i64,
    pub conversion_count: i64,
}
```

**Service Call:** `LinkAnalyticsService::get_campaign_performance()`

### `content analytics journey`

Show content navigation graph.

```bash
content analytics journey
content analytics journey --limit 50
```

**Args:**
```rust
#[derive(Args)]
pub struct JourneyArgs {
    #[arg(long, default_value = "20")]
    pub limit: i64,

    #[arg(long, default_value = "0")]
    pub offset: i64,
}
```

**Output Type:**
```rust
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JourneyOutput {
    pub nodes: Vec<JourneyNode>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct JourneyNode {
    pub source_content_id: ContentId,
    pub target_url: String,
    pub click_count: i64,
}
```

**Service Call:** `LinkAnalyticsService::get_content_journey_map()`

## Dependencies

Add to `crates/entry/cli/Cargo.toml`:
```toml
systemprompt_core_content = { path = "../../domain/content" }
```

## Implementation Checklist

- [ ] Create `commands/content/mod.rs` with `ContentCommands` enum
- [ ] Create `commands/content/types.rs` with output types
- [ ] Implement `content list`
- [ ] Implement `content show`
- [ ] Implement `content search`
- [ ] Implement `content ingest`
- [ ] Implement `content delete`
- [ ] Implement `content delete-source`
- [ ] Implement `content popular`
- [ ] Create `commands/content/link/mod.rs`
- [ ] Implement `content link generate`
- [ ] Implement `content link show`
- [ ] Implement `content link list`
- [ ] Implement `content link performance`
- [ ] Create `commands/content/analytics/mod.rs`
- [ ] Implement `content analytics clicks`
- [ ] Implement `content analytics campaign`
- [ ] Implement `content analytics journey`
- [ ] Add `Content` variant to main `Commands` enum in `lib.rs`
- [ ] Update CLI README with content commands

## Verification

```bash
# List content
systemprompt content list
systemprompt content list --source blog --json

# Show content
systemprompt content show content_abc123
systemprompt content show my-article --source blog

# Search content
systemprompt content search "kubernetes"
systemprompt content search "rust" --category tutorials

# Ingest content
systemprompt content ingest ./posts --source blog
systemprompt content ingest ./docs --source docs --recursive --override

# Delete content
systemprompt content delete content_abc123 --yes
systemprompt content delete-source old-blog --yes

# Popular content
systemprompt content popular --source blog --days 7

# Link generation
systemprompt content link generate --url https://example.com --campaign test
systemprompt content link show abc123
systemprompt content link list --campaign test
systemprompt content link performance link_abc123

# Analytics
systemprompt content analytics clicks link_abc123
systemprompt content analytics campaign test
systemprompt content analytics journey
```
