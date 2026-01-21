use crate::error::ContentError;
use crate::models::{
    CampaignLink, CampaignPerformance, ContentJourneyNode, LinkClick, LinkPerformance,
    RecordClickParams, TrackClickParams,
};
use crate::repository::{LinkAnalyticsRepository, LinkRepository};
use chrono::Utc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{CampaignId, ContentId, LinkClickId, LinkId};

const DEFAULT_JOURNEY_LIMIT: i64 = 50;
const DEFAULT_CLICKS_LIMIT: i64 = 100;

#[derive(Debug)]
pub struct LinkAnalyticsService {
    link_repo: LinkRepository,
    analytics_repo: LinkAnalyticsRepository,
}

impl LinkAnalyticsService {
    pub fn new(db: &DbPool) -> Result<Self, ContentError> {
        Ok(Self {
            link_repo: LinkRepository::new(db)?,
            analytics_repo: LinkAnalyticsRepository::new(db)?,
        })
    }

    pub async fn track_click(&self, params: &TrackClickParams) -> Result<LinkClick, ContentError> {
        let is_first_click = !self
            .analytics_repo
            .check_session_clicked_link(&params.link_id, &params.session_id)
            .await?;

        let click_id = LinkClickId::generate();
        let clicked_at = Utc::now();

        let record_params = RecordClickParams::new(
            click_id.clone(),
            params.link_id.clone(),
            params.session_id.clone(),
            clicked_at,
        )
        .with_user_id(params.user_id.clone())
        .with_context_id(params.context_id.clone())
        .with_task_id(params.task_id.clone())
        .with_referrer_page(params.referrer_page.clone())
        .with_referrer_url(params.referrer_url.clone())
        .with_user_agent(params.user_agent.clone())
        .with_ip_address(params.ip_address.clone())
        .with_device_type(params.device_type.clone())
        .with_country(params.country.clone())
        .with_is_first_click(is_first_click)
        .with_is_conversion(false);

        self.analytics_repo.record_click(&record_params).await?;

        self.analytics_repo
            .increment_link_clicks(&params.link_id, is_first_click)
            .await?;

        Ok(LinkClick {
            id: click_id,
            link_id: params.link_id.clone(),
            session_id: params.session_id.clone(),
            user_id: params.user_id.clone(),
            context_id: params.context_id.clone(),
            task_id: params.task_id.clone(),
            referrer_page: params.referrer_page.clone(),
            referrer_url: params.referrer_url.clone(),
            clicked_at: Some(clicked_at),
            user_agent: params.user_agent.clone(),
            ip_address: params.ip_address.clone(),
            device_type: params.device_type.clone(),
            country: params.country.clone(),
            is_first_click: Some(is_first_click),
            is_conversion: Some(false),
            conversion_at: None,
            time_on_page_seconds: None,
            scroll_depth_percent: None,
        })
    }

    pub async fn get_link_performance(
        &self,
        link_id: &LinkId,
    ) -> Result<Option<LinkPerformance>, ContentError> {
        Ok(self.analytics_repo.get_link_performance(link_id).await?)
    }

    pub async fn get_campaign_performance(
        &self,
        campaign_id: &CampaignId,
    ) -> Result<Option<CampaignPerformance>, ContentError> {
        Ok(self
            .analytics_repo
            .get_campaign_performance(campaign_id)
            .await?)
    }

    pub async fn get_content_journey_map(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<ContentJourneyNode>, ContentError> {
        let limit = limit.unwrap_or(DEFAULT_JOURNEY_LIMIT);
        let offset = offset.unwrap_or(0);
        Ok(self
            .analytics_repo
            .get_content_journey_map(limit, offset)
            .await?)
    }

    pub async fn get_link_clicks(
        &self,
        link_id: &LinkId,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<LinkClick>, ContentError> {
        let limit = limit.unwrap_or(DEFAULT_CLICKS_LIMIT);
        let offset = offset.unwrap_or(0);
        Ok(self
            .analytics_repo
            .get_clicks_by_link(link_id, limit, offset)
            .await?)
    }

    pub async fn get_links_by_campaign(
        &self,
        campaign_id: &CampaignId,
    ) -> Result<Vec<CampaignLink>, ContentError> {
        Ok(self.link_repo.list_links_by_campaign(campaign_id).await?)
    }

    pub async fn get_links_by_source_content(
        &self,
        source_content_id: &ContentId,
    ) -> Result<Vec<CampaignLink>, ContentError> {
        Ok(self
            .link_repo
            .list_links_by_source_content(source_content_id)
            .await?)
    }
}
