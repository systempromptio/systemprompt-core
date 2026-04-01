//! Unit tests for FileCategory enum

use systemprompt_files::FileCategory;

#[test]
fn test_file_category_storage_subdir_image() {
    assert_eq!(FileCategory::Image.storage_subdir(), "images");
}

#[test]
fn test_file_category_storage_subdir_document() {
    assert_eq!(FileCategory::Document.storage_subdir(), "documents");
}

#[test]
fn test_file_category_storage_subdir_audio() {
    assert_eq!(FileCategory::Audio.storage_subdir(), "audio");
}

#[test]
fn test_file_category_storage_subdir_video() {
    assert_eq!(FileCategory::Video.storage_subdir(), "video");
}

#[test]
fn test_file_category_display_name_image() {
    assert_eq!(FileCategory::Image.display_name(), "image");
}

#[test]
fn test_file_category_display_name_document() {
    assert_eq!(FileCategory::Document.display_name(), "document");
}

#[test]
fn test_file_category_display_name_audio() {
    assert_eq!(FileCategory::Audio.display_name(), "audio");
}

#[test]
fn test_file_category_display_name_video() {
    assert_eq!(FileCategory::Video.display_name(), "video");
}

#[test]
fn test_file_category_clone() {
    let category = FileCategory::Image;
    let cloned = category;
    assert_eq!(category, cloned);
}

#[test]
fn test_file_category_debug() {
    let debug_str = format!("{:?}", FileCategory::Document);
    assert!(debug_str.contains("Document"));
}

#[test]
fn test_file_category_equality() {
    assert_eq!(FileCategory::Image, FileCategory::Image);
    assert_ne!(FileCategory::Image, FileCategory::Document);
}
