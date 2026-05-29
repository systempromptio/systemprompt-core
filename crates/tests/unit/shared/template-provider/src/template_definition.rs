use std::path::PathBuf;

use systemprompt_provider_contracts::{TemplateDefinition, TemplateSource};

mod embedded_constructor {
    use super::*;

    #[test]
    fn sets_name_and_embedded_source() {
        let def = TemplateDefinition::embedded("my-template", "<!DOCTYPE html>");
        assert_eq!(def.name, "my-template");
        assert!(matches!(def.source, TemplateSource::Embedded("<!DOCTYPE html>")));
    }

    #[test]
    fn sets_default_priority_100() {
        let def = TemplateDefinition::embedded("t", "content");
        assert_eq!(def.priority, 100);
    }

    #[test]
    fn sets_empty_content_types() {
        let def = TemplateDefinition::embedded("t", "content");
        assert!(def.content_types.is_empty());
    }
}

mod file_constructor {
    use super::*;

    #[test]
    fn sets_name_and_file_source() {
        let def = TemplateDefinition::file("page", "templates/page.html");
        assert_eq!(def.name, "page");
        assert!(matches!(def.source, TemplateSource::File(_)));
    }

    #[test]
    fn file_path_is_preserved() {
        let def = TemplateDefinition::file("page", "templates/page.html");
        if let TemplateSource::File(p) = &def.source {
            assert_eq!(p, &PathBuf::from("templates/page.html"));
        } else {
            panic!("expected File source");
        }
    }

    #[test]
    fn accepts_pathbuf_directly() {
        let def = TemplateDefinition::file("x", PathBuf::from("/abs/path.html"));
        if let TemplateSource::File(p) = &def.source {
            assert_eq!(p, &PathBuf::from("/abs/path.html"));
        } else {
            panic!("expected File source");
        }
    }
}

mod directory_constructor {
    use super::*;

    #[test]
    fn sets_name_and_directory_source() {
        let def = TemplateDefinition::directory("layouts", "templates/layouts");
        assert_eq!(def.name, "layouts");
        assert!(matches!(def.source, TemplateSource::Directory(_)));
    }

    #[test]
    fn directory_path_is_preserved() {
        let def = TemplateDefinition::directory("d", "some/dir");
        if let TemplateSource::Directory(p) = &def.source {
            assert_eq!(p, &PathBuf::from("some/dir"));
        } else {
            panic!("expected Directory source");
        }
    }
}

mod with_priority {
    use super::*;

    #[test]
    fn overrides_default_priority() {
        let def = TemplateDefinition::embedded("t", "c").with_priority(50);
        assert_eq!(def.priority, 50);
    }

    #[test]
    fn zero_priority_accepted() {
        let def = TemplateDefinition::embedded("t", "c").with_priority(0);
        assert_eq!(def.priority, 0);
    }

    #[test]
    fn high_priority_accepted() {
        let def = TemplateDefinition::embedded("t", "c").with_priority(u32::MAX);
        assert_eq!(def.priority, u32::MAX);
    }

    #[test]
    fn preserves_name_and_source() {
        let def = TemplateDefinition::embedded("myname", "content").with_priority(200);
        assert_eq!(def.name, "myname");
        assert!(matches!(def.source, TemplateSource::Embedded("content")));
    }
}

mod for_content_types {
    use super::*;

    #[test]
    fn sets_content_types_vec() {
        let def = TemplateDefinition::embedded("t", "c")
            .for_content_types(vec!["blog".to_owned(), "guide".to_owned()]);
        assert_eq!(def.content_types, vec!["blog", "guide"]);
    }

    #[test]
    fn empty_vec_leaves_content_types_empty() {
        let def = TemplateDefinition::embedded("t", "c").for_content_types(vec![]);
        assert!(def.content_types.is_empty());
    }

    #[test]
    fn for_content_type_appends_single() {
        let def = TemplateDefinition::embedded("t", "c").for_content_type("docs");
        assert_eq!(def.content_types, vec!["docs"]);
    }

    #[test]
    fn for_content_type_chained_appends_each() {
        let def = TemplateDefinition::embedded("t", "c")
            .for_content_type("a")
            .for_content_type("b")
            .for_content_type("c");
        assert_eq!(def.content_types, vec!["a", "b", "c"]);
    }

    #[test]
    fn for_content_types_then_single_appends() {
        let def = TemplateDefinition::embedded("t", "c")
            .for_content_types(vec!["x".to_owned()])
            .for_content_type("y");
        assert_eq!(def.content_types, vec!["x", "y"]);
    }
}

mod debug_impl {
    use super::*;

    #[test]
    fn template_definition_is_debug() {
        let def = TemplateDefinition::embedded("tpl", "body");
        let s = format!("{:?}", def);
        assert!(s.contains("tpl"));
    }

    #[test]
    fn template_source_embedded_is_debug() {
        let src = TemplateSource::Embedded("hello");
        let s = format!("{:?}", src);
        assert!(s.contains("Embedded"));
    }

    #[test]
    fn template_source_file_is_debug() {
        let src = TemplateSource::File(PathBuf::from("a.html"));
        let s = format!("{:?}", src);
        assert!(s.contains("File"));
    }

    #[test]
    fn template_source_directory_is_debug() {
        let src = TemplateSource::Directory(PathBuf::from("dir"));
        let s = format!("{:?}", src);
        assert!(s.contains("Directory"));
    }
}

mod clone_impl {
    use super::*;

    #[test]
    fn template_definition_clones() {
        let def = TemplateDefinition::embedded("t", "c")
            .with_priority(42)
            .for_content_type("blog");
        let cloned = def.clone();
        assert_eq!(cloned.name, "t");
        assert_eq!(cloned.priority, 42);
        assert_eq!(cloned.content_types, vec!["blog"]);
    }

    #[test]
    fn template_source_clones() {
        let src = TemplateSource::Embedded("data");
        let cloned = src.clone();
        assert!(matches!(cloned, TemplateSource::Embedded("data")));
    }
}
