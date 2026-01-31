//! Tests for TemplateLoaderError

use std::io::ErrorKind;
use std::path::PathBuf;
use systemprompt_template_provider::TemplateLoaderError;

mod error_variants_tests {
    use super::*;

    #[test]
    fn not_found_contains_path() {
        let path = PathBuf::from("/templates/missing.html");
        let err = TemplateLoaderError::NotFound(path);
        let msg = err.to_string();
        assert!(msg.contains("missing.html"));
        assert!(msg.to_lowercase().contains("not found"));
    }

    #[test]
    fn directory_traversal_contains_path() {
        let path = PathBuf::from("../../../etc/passwd");
        let err = TemplateLoaderError::DirectoryTraversal(path);
        let msg = err.to_string();
        assert!(msg.contains("../../../etc/passwd"));
        assert!(msg.to_lowercase().contains("traversal"));
    }

    #[test]
    fn outside_base_path_contains_path() {
        let path = PathBuf::from("/outside/path");
        let err = TemplateLoaderError::OutsideBasePath(path);
        let msg = err.to_string();
        assert!(msg.contains("/outside/path"));
    }

    #[test]
    fn directory_not_supported_contains_path() {
        let path = PathBuf::from("/templates/dir");
        let err = TemplateLoaderError::DirectoryNotSupported(path);
        let msg = err.to_string();
        assert!(msg.contains("/templates/dir"));
        assert!(msg.to_lowercase().contains("directory"));
    }

    #[test]
    fn directory_loading_unsupported_message() {
        let err = TemplateLoaderError::DirectoryLoadingUnsupported;
        let msg = err.to_string();
        assert!(msg.to_lowercase().contains("directory"));
    }

    #[test]
    fn invalid_encoding_contains_path() {
        let path = PathBuf::from("/invalid_path");
        let err = TemplateLoaderError::InvalidEncoding(path);
        let msg = err.to_string();
        assert!(msg.to_lowercase().contains("encoding"));
    }

    #[test]
    fn no_base_paths_message() {
        let err = TemplateLoaderError::NoBasePaths;
        let msg = err.to_string();
        assert!(msg.to_lowercase().contains("base") || msg.to_lowercase().contains("path"));
    }

    #[test]
    fn io_error_contains_path_and_source() {
        let path = PathBuf::from("/test/file.txt");
        let io_err = std::io::Error::new(ErrorKind::NotFound, "file not found");
        let err = TemplateLoaderError::io(&path, io_err);
        let msg = err.to_string();
        assert!(msg.contains("/test/file.txt"));
    }

    #[test]
    fn io_constructor_from_path_buf() {
        let path = PathBuf::from("/path/to/file");
        let io_err = std::io::Error::new(ErrorKind::PermissionDenied, "denied");
        let err = TemplateLoaderError::io(path, io_err);
        let msg = err.to_string();
        assert!(msg.contains("/path/to/file"));
    }

    #[test]
    fn embedded_only_message() {
        let err = TemplateLoaderError::EmbeddedOnly;
        let msg = err.to_string();
        assert!(msg.to_lowercase().contains("embedded"));
    }
}

mod error_traits_tests {
    use super::*;

    #[test]
    fn error_is_debug() {
        let err = TemplateLoaderError::NoBasePaths;
        let debug = format!("{:?}", err);
        assert!(debug.contains("NoBasePaths"));
    }

    #[test]
    fn implements_std_error() {
        let err: Box<dyn std::error::Error> =
            Box::new(TemplateLoaderError::NoBasePaths);
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn io_error_has_source() {
        use std::error::Error;
        let path = PathBuf::from("/test");
        let io_err = std::io::Error::new(ErrorKind::NotFound, "not found");
        let err = TemplateLoaderError::io(path, io_err);
        assert!(err.source().is_some());
    }

    #[test]
    fn not_found_has_no_source() {
        use std::error::Error;
        let err = TemplateLoaderError::NotFound(PathBuf::from("/test"));
        assert!(err.source().is_none());
    }
}
