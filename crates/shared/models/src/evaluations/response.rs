use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AiEvaluationResponse {
    pub agent_goal: String,
    pub goal_achieved: String,
    pub goal_achievement_confidence: f64,
    pub goal_achievement_notes: Option<String>,

    pub primary_category: String,
    pub topics_discussed: String,
    pub keywords: String,

    pub user_satisfied: i32,
    pub conversation_quality: i32,
    pub quality_notes: Option<String>,
    pub issues_encountered: Option<String>,

    pub completion_status: String,
    pub overall_score: f64,
    pub evaluation_summary: String,
}
