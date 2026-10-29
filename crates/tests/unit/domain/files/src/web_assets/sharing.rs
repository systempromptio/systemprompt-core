use super::{assets_root, own_scripts};

fn asset(name: &str) -> String {
    std::fs::read_to_string(assets_root().join("js").join(name)).expect("asset")
}

#[test]
fn no_script_uses_native_dialogs_or_exceeds_the_line_limit() {
    for (name, text) in own_scripts() {
        for banned in ["alert(", "confirm(", "prompt("] {
            let native = text.match_indices(banned).any(|(at, _)| {
                at == 0
                    || !text[..at].ends_with(['.', '_'])
                        && !text[..at].ends_with(char::is_alphanumeric)
            });
            assert!(!native, "{name} calls {banned}");
        }
        assert!(text.lines().count() <= 150, "{name} is over 150 lines");
    }
}

#[test]
fn the_share_dialog_is_read_only_below_share() {
    let dialog = asset("sp-share-dialog.js");
    assert!(dialog.contains("RANK[this.access?.my_access] >= RANK.share"));
    assert!(dialog.contains("if (!this.canShare) { return"));
    assert!(dialog.contains("aria-label=\"Who has access\""));
}

#[test]
fn transfer_and_revoke_all_confirm_in_place() {
    let dialog = asset("sp-share-dialog.js");
    assert!(dialog.contains("this.on(\"action:revoke-all\", () => this.askToConfirm("));
    assert!(dialog.contains("this.on(\"submit:transfer\", (form) => this.askToConfirm("));
    assert!(dialog.contains("role=\"alertdialog\""));
    assert!(dialog.contains("data-action=\"confirm\""));
    assert!(dialog.contains("<sp-subject-picker id=\"transfer-subject\" kinds=\"user\""));
}

#[test]
fn the_share_form_carries_a_bounded_message_and_the_history_panel() {
    let dialog = asset("sp-share-dialog.js");
    assert!(dialog.contains("const MAX_MESSAGE = 1024;"));
    assert!(dialog.contains("name=\"message\""));
    assert!(dialog.contains("getAccess(this.documentId, { history: this.isHistoryOpen })"));
    let view = asset("documents-share-view.js");
    assert!(view.contains("from folder"));
    assert!(view.contains("data-expired"));
    assert!(view.contains("data-action=\"grant-request\""));
    assert!(view.contains("data-action=\"toggle-history\""));
}

#[test]
fn the_subject_picker_is_a_debounced_keyboard_listbox() {
    let picker = asset("sp-subject-picker.js");
    assert!(picker.contains("const DEBOUNCE_MS = 200;"));
    assert!(picker.contains("role=\"combobox\""));
    assert!(picker.contains("role=\"listbox\""));
    assert!(picker.contains("ArrowDown"));
    assert!(picker.contains("aria-activedescendant"));
    assert!(picker.contains("if (asked !== this.text.trim()) { return; }"));
    let api = asset("documents-api.js");
    assert!(api.contains("`${DOCUMENTS}/subjects${query({ kind, q, limit })}`"));
    assert!(api.contains("/access-requests/${encodeURIComponent(requestId)}/${decision}"));
}
