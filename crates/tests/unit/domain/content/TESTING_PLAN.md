# systemprompt-content Unit Tests

## Crate Overview
Content ingestion, search, and analytics. Handles paper/blog content parsing, metadata validation, search functionality, and link tracking.

## Source Files
- `src/services/ingestion/` - IngestionService
- `src/services/search/` - SearchService
- `src/services/link/` - LinkAnalyticsService, LinkGenerationService
- `src/services/validation/` - Validation services
- `src/repository/content/` - ContentRepository
- `src/repository/search/` - SearchRepository
- `src/repository/link/` - LinkAnalyticsRepository

## Test Plan

### Ingestion Service Tests
- `test_ingest_markdown_content` - Markdown parsing
- `test_ingest_yaml_frontmatter` - Frontmatter extraction
- `test_ingest_metadata_extraction` - Metadata handling
- `test_ingest_validation` - Content validation

### Search Service Tests
- `test_search_query_execution` - Execute query
- `test_search_filtering` - Apply filters
- `test_search_pagination` - Pagination handling
- `test_search_result_ranking` - Result ranking

### Link Generation Tests
- `test_link_generation_utm_params` - UTM parameters
- `test_link_generation_campaign` - Campaign links
- `test_link_generation_validation` - Link validation

### Link Analytics Tests
- `test_link_analytics_click_tracking` - Track clicks
- `test_link_analytics_performance` - Performance metrics
- `test_link_analytics_journey` - Journey analysis

### Validation Tests
- `test_validate_frontmatter` - Frontmatter validation
- `test_validate_section_structure` - Section validation
- `test_validate_metadata` - Metadata validation

## Mocking Requirements
- Mock database
- Mock search engine

## Test Fixtures Needed
- Sample markdown content
- Sample YAML frontmatter
- Sample search queries
