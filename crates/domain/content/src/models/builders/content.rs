use chrono::{DateTime, Utc};
use systemprompt_identifiers::{CategoryId, ContentId, SourceId};

#[derive(Debug, Clone)]
pub enum CategoryIdUpdate {
    Unchanged,
    Clear,
    Set(CategoryId),
}

impl From<Option<Option<CategoryId>>> for CategoryIdUpdate {
    fn from(value: Option<Option<CategoryId>>) -> Self {
        match value {
            None => Self::Unchanged,
            Some(None) => Self::Clear,
            Some(Some(id)) => Self::Set(id),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CreateContentParams {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub author: String,
    pub published_at: DateTime<Utc>,
    pub keywords: String,
    pub kind: String,
    pub image: Option<String>,
    pub category_id: Option<CategoryId>,
    pub source_id: SourceId,
    pub version_hash: String,
    pub links: serde_json::Value,
    pub public: bool,
}

impl CreateContentParams {
    pub fn new(
        slug: String,
        title: String,
        description: String,
        body: String,
        source_id: SourceId,
    ) -> Self {
        Self {
            slug,
            title,
            description,
            body,
            author: String::new(),
            published_at: Utc::now(),
            keywords: String::new(),
            kind: "article".into(),
            image: None,
            category_id: None,
            source_id,
            version_hash: String::new(),
            links: serde_json::Value::Array(vec![]),
            public: true,
        }
    }

    pub fn with_author(mut self, author: String) -> Self {
        self.author = author;
        self
    }

    pub const fn with_published_at(mut self, published_at: DateTime<Utc>) -> Self {
        self.published_at = published_at;
        self
    }

    pub fn with_keywords(mut self, keywords: String) -> Self {
        self.keywords = keywords;
        self
    }

    pub fn with_kind(mut self, kind: String) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_image(mut self, image: Option<String>) -> Self {
        self.image = image;
        self
    }

    pub fn with_category_id(mut self, category_id: Option<CategoryId>) -> Self {
        self.category_id = category_id;
        self
    }

    pub fn with_version_hash(mut self, version_hash: String) -> Self {
        self.version_hash = version_hash;
        self
    }

    pub fn with_links(mut self, links: serde_json::Value) -> Self {
        self.links = links;
        self
    }

    pub const fn with_public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }
}

#[derive(Debug, Clone)]
pub struct UpdateContentParams {
    pub id: ContentId,
    pub title: String,
    pub description: String,
    pub body: String,
    pub keywords: String,
    pub image: Option<String>,
    pub version_hash: String,
    pub category_id: CategoryIdUpdate,
    pub public: Option<bool>,
    pub kind: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub links: Option<serde_json::Value>,
}

impl UpdateContentParams {
    pub const fn new(id: ContentId, title: String, description: String, body: String) -> Self {
        Self {
            id,
            title,
            description,
            body,
            keywords: String::new(),
            image: None,
            version_hash: String::new(),
            category_id: CategoryIdUpdate::Unchanged,
            public: None,
            kind: None,
            author: None,
            published_at: None,
            links: None,
        }
    }

    pub fn with_keywords(mut self, keywords: String) -> Self {
        self.keywords = keywords;
        self
    }

    pub fn with_image(mut self, image: Option<String>) -> Self {
        self.image = image;
        self
    }

    pub fn with_version_hash(mut self, version_hash: String) -> Self {
        self.version_hash = version_hash;
        self
    }

    pub fn with_category_id(mut self, category_id: impl Into<CategoryIdUpdate>) -> Self {
        self.category_id = category_id.into();
        self
    }

    pub const fn with_public(mut self, public: Option<bool>) -> Self {
        self.public = public;
        self
    }

    pub fn with_kind(mut self, kind: Option<String>) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_author(mut self, author: Option<String>) -> Self {
        self.author = author;
        self
    }

    pub const fn with_published_at(mut self, published_at: Option<DateTime<Utc>>) -> Self {
        self.published_at = published_at;
        self
    }

    pub fn with_links(mut self, links: Option<serde_json::Value>) -> Self {
        self.links = links;
        self
    }
}
