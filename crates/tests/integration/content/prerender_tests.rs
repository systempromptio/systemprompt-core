use std::path::PathBuf;

fn determine_output_dir(dist_dir: &PathBuf, url_pattern: &str, slug: &str) -> PathBuf {
    let path = url_pattern.replace("{slug}", slug);
    let path = path.trim_start_matches('/');
    dist_dir.join(path)
}

#[test]
fn test_determine_output_dir() {
    let dist = PathBuf::from("/app/dist");
    let pattern = "/blog/{slug}";
    let result = determine_output_dir(&dist, pattern, "hello-world");
    assert_eq!(result, PathBuf::from("/app/dist/blog/hello-world"));
}

#[test]
fn test_determine_output_dir_trailing_slash() {
    let dist = PathBuf::from("/app/dist");
    let pattern = "/blog/{slug}/";
    let result = determine_output_dir(&dist, pattern, "hello-world");
    assert_eq!(result, PathBuf::from("/app/dist/blog/hello-world/"));
}

#[test]
fn test_determine_output_dir_root() {
    let dist = PathBuf::from("/app/dist");
    let pattern = "/{slug}";
    let result = determine_output_dir(&dist, pattern, "hello-world");
    assert_eq!(result, PathBuf::from("/app/dist/hello-world"));
}
