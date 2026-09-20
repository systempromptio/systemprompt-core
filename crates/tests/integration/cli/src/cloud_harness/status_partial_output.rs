//! Partial cloud status preserves healthy tenants and marks failed probes
//! unknown.

use std::io::{Read, Seek, SeekFrom};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;
use systemprompt_cli::cloud::{self, CloudCommands};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, ResponseTemplate};

use super::{FAR_FUTURE_JWT, OTHER_TENANT_ID, TENANT_ID, USER_EMAIL, enter, json_ctx};

const HELPER: &str = "cloud_harness::status_partial_output::mixed_status_helper";

#[tokio::test]
#[ignore = "re-executed by mixed_status_keeps_healthy_tenant_and_marks_failed_probe_unknown"]
async fn mixed_status_helper() {
    let env = enter().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/tenants"))
        .and(header("authorization", format!("Bearer {FAR_FUTURE_JWT}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [
                {"id": TENANT_ID, "name": "Harness Prod"},
                {"id": OTHER_TENANT_ID, "name": "Unavailable Sibling"}
            ]
        })))
        .expect(1)
        .mount(env.server())
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{TENANT_ID}/status")))
        .and(header("authorization", "Bearer tenant_bearer"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "status": "running",
                "app_url": "https://healthy.example.invalid",
                "message": "ready"
            }
        })))
        .expect(1)
        .mount(env.server())
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/tenants/{OTHER_TENANT_ID}/status")))
        .and(header("authorization", "Bearer tenant_bearer"))
        .respond_with(ResponseTemplate::new(503).set_body_string("owned status unavailable"))
        .expect(1)
        .mount(env.server())
        .await;

    println!("BEGIN_MIXED_STATUS");
    tokio::time::timeout(
        Duration::from_secs(10),
        cloud::execute(CloudCommands::Status, &json_ctx()),
    )
    .await
    .expect("mixed status completes within ten seconds")
    .expect("one failed tenant probe does not fail the command");
    println!("END_MIXED_STATUS");
}

fn bounded_helper_output() -> (String, String) {
    let stdout = tempfile::tempfile().expect("owned status stdout");
    let stderr = tempfile::tempfile().expect("owned status stderr");
    let child = Command::new(std::env::current_exe().expect("cloud harness test binary"))
        .args(["--exact", HELPER, "--ignored", "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(
            stdout.try_clone().expect("clone status stdout"),
        ))
        .stderr(Stdio::from(
            stderr.try_clone().expect("clone status stderr"),
        ))
        .spawn()
        .expect("spawn isolated status helper");
    let mut child = OwnedChild::new(child);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll status helper") {
            break status;
        }
        if Instant::now() >= deadline {
            panic!("mixed-status helper exceeded twenty seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    child.disarm();
    let read = |mut file: std::fs::File| {
        file.seek(SeekFrom::Start(0))
            .expect("rewind status capture");
        let mut text = String::new();
        file.read_to_string(&mut text).expect("read status capture");
        text
    };
    let stdout = read(stdout);
    let stderr = read(stderr);
    assert!(
        status.success(),
        "mixed-status helper failed; stdout={stdout}; stderr={stderr}"
    );
    (stdout, stderr)
}

fn section<'a>(card: &'a Value, heading: &str) -> &'a Value {
    card["sections"]
        .as_array()
        .expect("status card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .map(|section| &section["content"])
        .unwrap_or_else(|| panic!("missing {heading} section: {card}"))
}

#[test]
fn mixed_status_keeps_healthy_tenant_and_marks_failed_probe_unknown() {
    let (stdout, _stderr) = bounded_helper_output();
    let json = stdout
        .split_once("BEGIN_MIXED_STATUS")
        .and_then(|(_, tail)| tail.split_once("END_MIXED_STATUS"))
        .map(|(value, _)| value.trim())
        .unwrap_or_else(|| panic!("missing mixed-status markers: {stdout}"));
    let card: Value = serde_json::from_str(json).unwrap_or_else(|error| panic!("{error}: {json}"));
    assert_eq!(card["title"], "Cloud Status");
    assert_eq!(
        section(&card, "credentials"),
        &serde_json::json!({
            "authenticated": true,
            "user_email": USER_EMAIL,
            "token_expired": false
        })
    );
    assert_eq!(
        section(&card, "tenants"),
        &serde_json::json!([
            {
                "id": TENANT_ID,
                "name": "Harness Prod",
                "status": "running",
                "url": "https://healthy.example.invalid",
                "message": "ready",
                "configured_in_profile": true
            },
            {
                "id": OTHER_TENANT_ID,
                "name": "Unavailable Sibling",
                "status": "unknown",
                "configured_in_profile": false
            }
        ])
    );
}

struct OwnedChild(Option<Child>);

impl OwnedChild {
    const fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        self.0.as_mut().expect("owned child present").try_wait()
    }

    fn disarm(&mut self) {
        self.0.take();
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
