//! The full path the overlap and reservation checks compare is the path the
//! server serves, for every base kind (served paths from the baseline
//! `routes.tsv`).

use systemprompt_extension::router::{
    ApiBase, BasePath, BaseRef, GatewayPath, WebBase, full_path,
};

#[test]
fn the_full_path_of_every_base_kind_is_its_served_path() {
    let cases: [(BaseRef, &str, &str); 13] = [
        (BaseRef::Path(BasePath::Api(ApiBase::Analytics)), "/events", "/api/v1/analytics/events"),
        (BaseRef::Path(BasePath::Api(ApiBase::Content)), "/links", "/api/v1/content/links"),
        (BaseRef::Path(BasePath::Api(ApiBase::Notifications)), "/", "/api/v1/notifications"),
        (
            BaseRef::Path(BasePath::Api(ApiBase::AdminNotifications)),
            "/simulate",
            "/api/v1/admin/notifications/simulate",
        ),
        (BaseRef::Path(BasePath::Api(ApiBase::AgentRuns)), "/", "/api/v1/agent-runs"),
        (BaseRef::Path(BasePath::Api(ApiBase::Agents)), "/", "/api/v1/agents"),
        (BaseRef::Path(BasePath::Api(ApiBase::Slack)), "/commands", "/api/v1/slack/commands"),
        (BaseRef::Path(BasePath::Api(ApiBase::Teams)), "/messages", "/api/v1/teams/messages"),
        (
            BaseRef::Path(BasePath::Api(ApiBase::Extension("/api/v1/probe"))),
            "/x",
            "/api/v1/probe/x",
        ),
        (BaseRef::Path(BasePath::Web(WebBase::TrackEngagement)), "/", "/track/engagement"),
        (BaseRef::Path(BasePath::Web(WebBase::Root)), "/r/{short_code}", "/r/{short_code}"),
        (
            BaseRef::Gateway(GatewayPath::BridgeNotifications),
            "/bridge/notifications",
            "/v1/bridge/notifications",
        ),
        (
            BaseRef::Gateway(GatewayPath::BridgeNotifications),
            "/bridge/notifications/read",
            "/v1/bridge/notifications/read",
        ),
    ];
    for (base, path, served) in cases {
        assert_eq!(full_path(base, path), served, "{base:?} {path}");
    }
}
