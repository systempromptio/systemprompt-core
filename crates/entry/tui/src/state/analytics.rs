use std::time::Instant;

use systemprompt_models::admin::{
    ActivityTrend, BotTrafficStats, BrowserBreakdown, ContentStat, DeviceBreakdown,
    GeographicBreakdown, RecentConversation, UserMetricsWithTrends,
};

#[derive(Debug)]
pub struct AnalyticsState {
    pub user_metrics: Option<UserMetricsWithTrends>,
    pub content_stats: Vec<ContentStat>,
    pub recent_conversations: Vec<RecentConversation>,
    pub activity_trends: Vec<ActivityTrend>,
    pub traffic_data: TrafficData,
    pub loading: bool,
    pub last_refresh: Option<Instant>,
    pub scroll_offset: usize,
    pub selected_section: AnalyticsSection,
    pub active_view: AnalyticsView,
}

#[derive(Debug, Clone, Default)]
pub struct TrafficData {
    pub browsers: Vec<BrowserBreakdown>,
    pub devices: Vec<DeviceBreakdown>,
    pub countries: Vec<GeographicBreakdown>,
    pub bot_traffic: BotTrafficStats,
}

impl AnalyticsState {
    pub fn new() -> Self {
        Self {
            user_metrics: None,
            content_stats: Vec::new(),
            recent_conversations: Vec::new(),
            activity_trends: Vec::new(),
            traffic_data: TrafficData::default(),
            loading: true,
            last_refresh: None,
            scroll_offset: 0,
            selected_section: AnalyticsSection::UserMetrics,
            active_view: AnalyticsView::Content,
        }
    }

    pub fn update(&mut self, data: AnalyticsData) {
        self.user_metrics = data.user_metrics;
        self.content_stats = data.content_stats;
        self.recent_conversations = data.recent_conversations;
        self.activity_trends = data.activity_trends;
        if let Some(traffic) = data.traffic_data {
            self.traffic_data = traffic;
        }
        self.loading = false;
        self.last_refresh = Some(Instant::now());
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
    }

    pub fn next_view(&mut self) {
        self.active_view = match self.active_view {
            AnalyticsView::Content => AnalyticsView::Conversations,
            AnalyticsView::Conversations => AnalyticsView::Traffic,
            AnalyticsView::Traffic => AnalyticsView::Content,
        };
        self.scroll_offset = 0;
    }

    pub fn prev_view(&mut self) {
        self.active_view = match self.active_view {
            AnalyticsView::Content => AnalyticsView::Traffic,
            AnalyticsView::Conversations => AnalyticsView::Content,
            AnalyticsView::Traffic => AnalyticsView::Conversations,
        };
        self.scroll_offset = 0;
    }

    pub fn next_section(&mut self) {
        self.selected_section = match self.selected_section {
            AnalyticsSection::UserMetrics => AnalyticsSection::ContentStats,
            AnalyticsSection::ContentStats => AnalyticsSection::RecentConversations,
            AnalyticsSection::RecentConversations => AnalyticsSection::UserMetrics,
        };
        self.scroll_offset = 0;
    }

    pub fn prev_section(&mut self) {
        self.selected_section = match self.selected_section {
            AnalyticsSection::UserMetrics => AnalyticsSection::RecentConversations,
            AnalyticsSection::ContentStats => AnalyticsSection::UserMetrics,
            AnalyticsSection::RecentConversations => AnalyticsSection::ContentStats,
        };
        self.scroll_offset = 0;
    }
}

impl Default for AnalyticsState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalyticsView {
    #[default]
    Content,
    Conversations,
    Traffic,
}

impl AnalyticsView {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Content => "Content",
            Self::Conversations => "Conversations",
            Self::Traffic => "Traffic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnalyticsSection {
    #[default]
    UserMetrics,
    ContentStats,
    RecentConversations,
}

#[derive(Debug, Clone)]
pub struct AnalyticsData {
    pub user_metrics: Option<UserMetricsWithTrends>,
    pub content_stats: Vec<ContentStat>,
    pub recent_conversations: Vec<RecentConversation>,
    pub activity_trends: Vec<ActivityTrend>,
    pub traffic_data: Option<TrafficData>,
}
