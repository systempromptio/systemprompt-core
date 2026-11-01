use http::Method;
use systemprompt_identifiers::DocumentUploadId;
use systemprompt_runtime::documents::upload_links::{
    UploadGrant, purpose_permits, required_grant, upload_purpose,
};

#[test]
fn each_upload_route_names_the_grant_it_needs() {
    assert_eq!(
        required_grant(&Method::PUT, "parts/3"),
        Some(UploadGrant::Write)
    );
    assert_eq!(
        required_grant(&Method::POST, "complete"),
        Some(UploadGrant::Write)
    );
    assert_eq!(required_grant(&Method::GET, ""), Some(UploadGrant::Write));
    assert_eq!(
        required_grant(&Method::HEAD, "content"),
        Some(UploadGrant::Download)
    );
}

#[test]
fn a_route_no_token_opens_names_no_grant() {
    assert_eq!(required_grant(&Method::PUT, "parts/"), None);
    assert_eq!(required_grant(&Method::PUT, "parts/1/2"), None);
    assert_eq!(required_grant(&Method::POST, "content"), None);
    assert_eq!(required_grant(&Method::DELETE, ""), None);
    assert_eq!(required_grant(&Method::GET, "complete"), None);
}

#[test]
fn a_token_opens_only_its_own_upload_and_grant() {
    let upload = DocumentUploadId::generate();
    let other = DocumentUploadId::generate();
    let write = upload_purpose(UploadGrant::Write, &upload);
    let download = upload_purpose(UploadGrant::Download, &upload);

    assert!(purpose_permits(
        Some(&write),
        &Method::PUT,
        "parts/1",
        &upload
    ));
    assert!(!purpose_permits(
        Some(&write),
        &Method::GET,
        "content",
        &upload
    ));
    assert!(purpose_permits(
        Some(&download),
        &Method::GET,
        "content",
        &upload
    ));
    assert!(!purpose_permits(
        Some(&download),
        &Method::POST,
        "complete",
        &upload
    ));
    assert!(!purpose_permits(
        Some(&write),
        &Method::PUT,
        "parts/1",
        &other
    ));
    assert!(!purpose_permits(None, &Method::PUT, "parts/1", &upload));
}
