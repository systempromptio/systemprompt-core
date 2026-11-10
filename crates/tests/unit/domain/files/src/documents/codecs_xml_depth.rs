use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::codecs::odf::OdtCodec;
use systemprompt_provider_contracts::documents::{CodecError, DocumentCodec, Projection};

use super::docx_fixture::package_with;
use super::odf_fixture::package;

const ODT_MIME: &str = "application/vnd.oasis.opendocument.text";

const W_NS: &str = r#"xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main""#;

fn nested_docx(depth: usize) -> Vec<u8> {
    let open = "<w:sdt><w:sdtContent>".repeat(depth / 2);
    let close = "</w:sdtContent></w:sdt>".repeat(depth / 2);
    package_with(&format!(
        "<?xml version=\"1.0\"?><w:document {W_NS}><w:body>{open}\
         <w:p><w:r><w:t>deep</w:t></w:r></w:p>{close}</w:body></w:document>"
    ))
}

fn nested_odt(depth: usize) -> Vec<u8> {
    let open = "<text:span>".repeat(depth);
    let close = "</text:span>".repeat(depth);
    package(
        ODT_MIME,
        &format!("<office:text><text:p>{open}deep{close}</text:p></office:text>"),
    )
}

fn on_small_stack(
    codec: &'static (dyn DocumentCodec + Sync),
    bytes: Vec<u8>,
) -> Result<Projection, CodecError> {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || codec.project(&bytes))
        .expect("spawn")
        .join()
        .expect("the parse does not overflow the stack")
}

fn assert_too_deep(result: Result<Projection, CodecError>) {
    match result {
        Err(CodecError::TooLarge { what, limit, .. }) => {
            assert_eq!(what, "XML element nesting depth");
            assert_eq!(limit, 256);
        },
        other => panic!("expected the nesting depth refusal, got {other:?}"),
    }
}

#[test]
fn a_docx_nested_past_the_depth_cap_is_refused_on_a_small_stack() {
    assert_too_deep(on_small_stack(&DocxCodec, nested_docx(200_000)));
}

#[test]
fn an_odt_nested_past_the_depth_cap_is_refused_on_a_small_stack() {
    assert_too_deep(on_small_stack(&OdtCodec, nested_odt(200_000)));
}

#[test]
fn moderate_nesting_still_parses() {
    let docx = on_small_stack(&DocxCodec, nested_docx(64)).expect("docx parses");
    assert!(format!("{docx:?}").contains("deep"));
    let odt = on_small_stack(&OdtCodec, nested_odt(64)).expect("odt parses");
    assert!(format!("{odt:?}").contains("deep"));
}
