use crate::error::ContentError;
use crate::models::{CampaignLink, CreateLinkParams, DestinationType, LinkType, UtmParams};
use crate::repository::LinkRepository;
use chrono::{DateTime, Utc};
use systemprompt_core_database::DbPool;
use systemprompt_identifiers::{CampaignId, ContentId};

#[derive(Debug)]
pub struct GenerateLinkParams {
    pub target_url: String,
    pub link_type: LinkType,
    pub campaign_id: Option<CampaignId>,
    pub campaign_name: Option<String>,
    pub source_content_id: Option<ContentId>,
    pub source_page: Option<String>,
    pub utm_params: Option<UtmParams>,
    pub link_text: Option<String>,
    pub link_position: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct GenerateContentLinkParams<'a> {
    pub target_url: &'a str,
    pub source_content_id: &'a ContentId,
    pub source_page: &'a str,
    pub link_text: Option<String>,
    pub link_position: Option<String>,
}

#[derive(Debug)]
pub struct LinkGenerationService {
    link_repo: LinkRepository,
}

impl LinkGenerationService {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            link_repo: LinkRepository::new(db)?,
        })
    }

    pub async fn generate_link(
        &self,
        params: GenerateLinkParams,
    ) -> Result<CampaignLink, ContentError> {
        let short_code = Self::generate_short_code();
        let destination_type = Self::determine_destination_type(&params.target_url);

        let utm_json = params
            .utm_params
            .as_ref()
            .map(UtmParams::to_json)
            .transpose()?;

        let create_params =
            CreateLinkParams::new(short_code, params.target_url, params.link_type.to_string())
                .with_source_content_id(params.source_content_id)
                .with_source_page(params.source_page)
                .with_campaign_id(params.campaign_id)
                .with_campaign_name(params.campaign_name)
                .with_utm_params(utm_json)
                .with_link_text(params.link_text)
                .with_link_position(params.link_position)
                .with_destination_type(Some(destination_type.to_string()))
                .with_expires_at(params.expires_at);

        let link = self.link_repo.create_link(&create_params).await?;

        Ok(link)
    }

    pub async fn generate_social_media_link(
        &self,
        target_url: &str,
        platform: &str,
        campaign_name: &str,
        source_content_id: Option<ContentId>,
    ) -> Result<CampaignLink, ContentError> {
        let campaign_id =
            CampaignId::new(format!("social_{}_{}", platform, Utc::now().timestamp()));

        let utm_params = UtmParams {
            source: Some(platform.to_string()),
            medium: Some("social".to_string()),
            campaign: Some(campaign_name.to_string()),
            term: None,
            content: source_content_id.as_ref().map(ToString::to_string),
        };

        self.generate_link(GenerateLinkParams {
            target_url: target_url.to_string(),
            link_type: LinkType::Both,
            campaign_id: Some(campaign_id),
            campaign_name: Some(campaign_name.to_string()),
            source_content_id,
            source_page: None,
            utm_params: Some(utm_params),
            link_text: None,
            link_position: None,
            expires_at: None,
        })
        .await
    }

    pub async fn generate_internal_content_link(
        &self,
        params: GenerateContentLinkParams<'_>,
    ) -> Result<CampaignLink, ContentError> {
        let campaign_id =
            CampaignId::new(format!("internal_navigation_{}", Utc::now().date_naive()));

        let utm_params = UtmParams {
            source: Some("internal".to_string()),
            medium: Some("content".to_string()),
            campaign: None,
            term: None,
            content: Some(params.source_content_id.to_string()),
        };

        self.generate_link(GenerateLinkParams {
            target_url: params.target_url.to_string(),
            link_type: LinkType::Utm,
            campaign_id: Some(campaign_id),
            campaign_name: Some("Internal Content Navigation".to_string()),
            source_content_id: Some(params.source_content_id.clone()),
            source_page: Some(params.source_page.to_string()),
            utm_params: Some(utm_params),
            link_text: params.link_text,
            link_position: params.link_position,
            expires_at: None,
        })
        .await
    }

    pub async fn generate_external_cta_link(
        &self,
        target_url: &str,
        campaign_name: &str,
        source_content_id: Option<ContentId>,
        link_text: Option<String>,
    ) -> Result<CampaignLink, ContentError> {
        let campaign_id = CampaignId::new(format!("external_cta_{}", Utc::now().timestamp()));

        let utm_params = UtmParams {
            source: Some("blog".to_string()),
            medium: Some("cta".to_string()),
            campaign: Some(campaign_name.to_string()),
            term: None,
            content: source_content_id.as_ref().map(ToString::to_string),
        };

        self.generate_link(GenerateLinkParams {
            target_url: target_url.to_string(),
            link_type: LinkType::Both,
            campaign_id: Some(campaign_id),
            campaign_name: Some(campaign_name.to_string()),
            source_content_id,
            source_page: None,
            utm_params: Some(utm_params),
            link_text,
            link_position: Some("cta".to_string()),
            expires_at: None,
        })
        .await
    }

    pub async fn generate_external_content_link(
        &self,
        params: GenerateContentLinkParams<'_>,
    ) -> Result<CampaignLink, ContentError> {
        let campaign_id = CampaignId::new(format!("social_share_{}", Utc::now().date_naive()));

        self.generate_link(GenerateLinkParams {
            target_url: params.target_url.to_string(),
            link_type: LinkType::Redirect,
            campaign_id: Some(campaign_id),
            campaign_name: Some("Social Share".to_string()),
            source_content_id: Some(params.source_content_id.clone()),
            source_page: Some(params.source_page.to_string()),
            utm_params: None,
            link_text: params.link_text,
            link_position: params.link_position,
            expires_at: None,
        })
        .await
    }

    pub async fn get_link_by_short_code(
        &self,
        short_code: &str,
    ) -> Result<Option<CampaignLink>, ContentError> {
        Ok(self.link_repo.get_link_by_short_code(short_code).await?)
    }

    pub fn build_trackable_url(link: &CampaignLink, base_url: &str) -> String {
        match link.link_type.as_str() {
            "redirect" | "both" => {
                format!("{}/r/{}", base_url, link.short_code)
            },
            _ => link.target_url.clone(),
        }
    }

    pub fn inject_utm_params(url: &str, utm_params: &UtmParams) -> String {
        let query_string = utm_params.to_query_string();
        if query_string.is_empty() {
            url.to_string()
        } else {
            let separator = if url.contains('?') { "&" } else { "?" };
            format!("{url}{separator}{query_string}")
        }
    }

    fn generate_short_code() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        const CODE_LENGTH: usize = 8;

        let mut rng = rand::thread_rng();
        (0..CODE_LENGTH)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    fn determine_destination_type(url: &str) -> DestinationType {
        if url.starts_with('/')
            || url.starts_with("http://localhost")
            || url.starts_with("https://localhost")
            || url.contains("tyingshoelaces.com")
            || url.contains("systemprompt.io")
        {
            DestinationType::Internal
        } else {
            DestinationType::External
        }
    }
}
