use systemprompt_models::mime;

use std::path::Path;

#[test]
fn webp_resolves_to_its_image_type() {
    assert_eq!(
        mime::http_content_type(Path::new("hero.webp")),
        "image/webp"
    );
    assert_eq!(mime::http_content_type(Path::new("icon.gif")), "image/gif");
    assert_eq!(
        mime::http_content_type(Path::new("shot.avif")),
        "image/avif"
    );
}

#[test]
fn extension_case_is_ignored() {
    assert_eq!(
        mime::http_content_type(Path::new("HERO.WEBP")),
        "image/webp"
    );
    assert_eq!(mime::from_path(Path::new("Photo.JPEG")), "image/jpeg");
}

#[test]
fn unknown_and_absent_extensions_fall_back_to_a_byte_stream() {
    assert_eq!(
        mime::http_content_type(Path::new("archive.zzz")),
        "application/octet-stream"
    );
    assert_eq!(
        mime::http_content_type(Path::new("README")),
        "application/octet-stream"
    );
}

#[test]
fn the_two_woff_formats_are_distinguished() {
    assert_eq!(mime::from_path(Path::new("a.woff")), "font/woff");
    assert_eq!(mime::from_path(Path::new("a.woff2")), "font/woff2");
}

#[test]
fn essence_and_served_forms_differ_for_text_only() {
    assert_eq!(mime::from_path(Path::new("a.css")), "text/css");
    assert_eq!(
        mime::http_content_type(Path::new("a.css")),
        "text/css; charset=utf-8"
    );
    assert_eq!(mime::from_path(Path::new("a.png")), "image/png");
    assert_eq!(mime::http_content_type(Path::new("a.png")), "image/png");
}

#[test]
fn charset_is_confined_to_text_types() {
    assert_eq!(
        mime::http_content_type(Path::new("a.json")),
        "application/json"
    );
    assert_eq!(
        mime::http_content_type(Path::new("a.yaml")),
        "application/yaml"
    );
    assert_eq!(mime::http_content_type(Path::new("a.svg")), "image/svg+xml");
}

#[test]
fn parameters_and_case_are_stripped_from_an_essence() {
    assert_eq!(mime::essence_of("Text/Plain; charset=utf-8"), "text/plain");
    assert_eq!(mime::essence_of("  image/PNG  "), "image/png");
    assert_eq!(mime::essence_of("image/webp"), "image/webp");
}

#[test]
fn accepted_upload_types_round_trip_to_an_extension() {
    for (mime_type, expected) in [
        ("image/jpeg", "jpg"),
        ("image/png", "png"),
        ("image/webp", "webp"),
        ("image/svg+xml", "svg"),
        ("image/tiff", "tiff"),
        ("application/pdf", "pdf"),
        ("text/plain", "txt"),
        ("application/json", "json"),
        ("audio/mpeg", "mp3"),
        ("audio/webm", "weba"),
        ("video/mp4", "mp4"),
        ("video/ogg", "ogv"),
    ] {
        assert_eq!(
            mime::extension_for(mime_type),
            Some(expected),
            "{mime_type} should resolve to {expected}"
        );
    }
}

#[test]
fn aliases_and_parameterised_types_resolve_to_an_extension() {
    assert_eq!(mime::extension_for("image/vnd.microsoft.icon"), Some("ico"));
    assert_eq!(mime::extension_for("text/xml"), Some("xml"));
    assert_eq!(mime::extension_for("audio/x-wav"), Some("wav"));
    assert_eq!(
        mime::extension_for("text/plain; charset=utf-8"),
        Some("txt")
    );
}

#[test]
fn an_unmapped_type_has_no_extension() {
    assert_eq!(mime::extension_for("application/x-not-real"), None);
}
