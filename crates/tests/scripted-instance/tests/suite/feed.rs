use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Response};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, DriveDeviceId};
use systemprompt_models::ApiPaths;
use systemprompt_models::documents::client::{CommitMeta, CommitOutcome, FeedManifest};
use systemprompt_models::documents::{DriveSyncToken, content_hash};
use systemprompt_scripted_instance::{ModelScript, ScriptedInstance};

use crate::support::{instance, signed_in};

pub(crate) const BOUNDARY: &str = "scripted-instance-boundary";

pub(crate) struct Feed {
    pub(crate) instance: ScriptedInstance,
    pub(crate) http: Client,
    pub(crate) token: String,
}

impl Feed {
    pub(crate) async fn open() -> Self {
        let instance = instance(ModelScript::empty()).await;
        let (_client, session) = signed_in(&instance).await;
        let http = Client::new();
        let device = session.device_token.unwrap();
        let traded: DriveSyncToken = http
            .post(format!(
                "{}{}",
                instance.base_url(),
                ApiPaths::BRIDGE_DRIVE_TOKEN
            ))
            .bearer_auth(device.expose())
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        Self {
            instance,
            http,
            token: traded.token,
        }
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.instance.base_url())
    }

    pub(crate) async fn manifest(&self) -> FeedManifest {
        let answer = self
            .http
            .get(self.url(ApiPaths::BRIDGE_DRIVE_MANIFEST))
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap();
        answer.json().await.unwrap()
    }

    pub(crate) async fn changes(&self, since: &str) -> Response {
        self.http
            .get(self.url(ApiPaths::BRIDGE_DRIVE_CHANGES))
            .query(&[("since", since)])
            .bearer_auth(&self.token)
            .send()
            .await
            .unwrap()
    }

    pub(crate) async fn commit(
        &self,
        document_id: Option<DocumentId>,
        path: &str,
        base: Option<DocumentRevisionId>,
        blob: &[u8],
    ) -> Response {
        let meta = CommitMeta {
            document_id,
            path: path.to_owned(),
            base_revision: base,
            content_sha256: content_hash(blob).to_string(),
            size_bytes: blob.len() as u64,
            device_id: DriveDeviceId::try_new("scripted-test-device").unwrap(),
            message: None,
        };
        let meta = serde_json::to_string(&meta).unwrap();
        let mut body = Vec::new();
        body.extend_from_slice(part("meta", "application/json").as_bytes());
        body.extend_from_slice(meta.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(part("blob", "application/octet-stream").as_bytes());
        body.extend_from_slice(blob);
        body.extend_from_slice(b"\r\n");
        self.post_parts(body).await
    }

    pub(crate) async fn post_parts(&self, mut body: Vec<u8>) -> Response {
        body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
        self.http
            .post(self.url(ApiPaths::BRIDGE_DRIVE_COMMIT))
            .bearer_auth(&self.token)
            .header(
                CONTENT_TYPE,
                format!("multipart/form-data; boundary={BOUNDARY}"),
            )
            .body(body)
            .send()
            .await
            .unwrap()
    }
}

pub(crate) fn part(name: &str, mime: &str) -> String {
    format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"; \
         filename=\"{name}\"\r\nContent-Type: {mime}\r\n\r\n"
    )
}

pub(crate) async fn committed(answer: Response) -> DocumentRevisionId {
    assert_eq!(answer.status().as_u16(), 200);
    match answer.json().await.unwrap() {
        CommitOutcome::Committed { revision_id } => revision_id,
        other => panic!("the commit lands: {other:?}"),
    }
}
