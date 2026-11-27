use std::path::Path;

use systemprompt_engine::lsp::language_id;

#[test]
fn maps_common_extensions() {
    assert_eq!(language_id(Path::new("src/main.RS")), "rust");
    assert_eq!(language_id(Path::new("a/b.tsx")), "typescriptreact");
    assert_eq!(language_id(Path::new("Makefile")), "plaintext");
}
