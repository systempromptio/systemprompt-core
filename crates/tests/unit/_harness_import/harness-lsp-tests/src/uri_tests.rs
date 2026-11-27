use std::path::PathBuf;

use harness_lsp::{PathStyle, decode_uri, encode_path, path_to_uri, uri_to_path};

const WINDOWS_ROUND_TRIP: &[(&str, &str)] = &[
    (r"C:\Users\me\a b.rs", "file:///C:/Users/me/a%20b.rs"),
    (r"D:\", "file:///D:/"),
    (
        r"C:\Ünï\#hash%.rs",
        "file:///C:/%C3%9Cn%C3%AF/%23hash%25.rs",
    ),
    (r"\\server\share\dir\f.rs", "file://server/share/dir/f.rs"),
    (r"C:\src\[x]\a+b@c.ts", "file:///C:/src/%5Bx%5D/a+b@c.ts"),
];

const WINDOWS_ENCODE_ONLY: &[(&str, &str)] = &[
    (r"c:\lower\drive.rs", "file:///C:/lower/drive.rs"),
    ("C:/mixed/slash.rs", "file:///C:/mixed/slash.rs"),
    (r"\\?\C:\long\path.rs", "file:///C:/long/path.rs"),
    (r"\\?\UNC\server\share\f.rs", "file://server/share/f.rs"),
    ("C:", "file:///C:/"),
];

const WINDOWS_DECODE_ONLY: &[(&str, &str)] = &[
    ("file:///c%3A/Users/me/x.rs", r"C:\Users\me\x.rs"),
    ("file:///c:/x", r"C:\x"),
    ("file:///C%3a", r"C:\"),
    ("FILE:///C:/x.rs?query#fragment", r"C:\x.rs"),
    ("file://localhost/C:/x.rs", r"C:\x.rs"),
    ("file:/C:/x.rs", r"C:\x.rs"),
    ("file://SERVER/share/a%20b.rs", r"\\SERVER\share\a b.rs"),
];

const POSIX_ROUND_TRIP: &[(&str, &str)] = &[
    ("/home/me/a b#c.rs", "file:///home/me/a%20b%23c.rs"),
    ("/", "file:///"),
    ("/tmp/naïve/x?.rs", "file:///tmp/na%C3%AFve/x%3F.rs"),
    ("/src/a:b/100%.rs", "file:///src/a:b/100%25.rs"),
];

const POSIX_DECODE_ONLY: &[(&str, Option<&str>)] = &[
    ("file://localhost/etc/hosts", Some("/etc/hosts")),
    ("file:///a/b.rs?q=1#L3", Some("/a/b.rs")),
    ("file://server/share/x", None),
    ("untitled:Untitled-1", None),
    ("https://example.com/a.rs", None),
    ("file:///%FF", None),
    ("file:///bad%2", Some("/bad%2")),
];

#[test]
fn windows_paths_round_trip() {
    for (path, uri) in WINDOWS_ROUND_TRIP {
        assert_eq!(encode_path(path, PathStyle::Windows), *uri, "encode {path}");
        assert_eq!(
            decode_uri(uri, PathStyle::Windows).as_deref(),
            Some(*path),
            "decode {uri}"
        );
    }
}

#[test]
fn windows_encoding_normalises() {
    for (path, uri) in WINDOWS_ENCODE_ONLY {
        assert_eq!(encode_path(path, PathStyle::Windows), *uri, "encode {path}");
    }
}

#[test]
fn windows_decoding_accepts_server_variants() {
    for (uri, path) in WINDOWS_DECODE_ONLY {
        assert_eq!(
            decode_uri(uri, PathStyle::Windows).as_deref(),
            Some(*path),
            "decode {uri}"
        );
    }
    assert_eq!(decode_uri("untitled:1", PathStyle::Windows), None);
}

#[test]
fn posix_paths_round_trip() {
    for (path, uri) in POSIX_ROUND_TRIP {
        assert_eq!(encode_path(path, PathStyle::Posix), *uri, "encode {path}");
        assert_eq!(
            decode_uri(uri, PathStyle::Posix).as_deref(),
            Some(*path),
            "decode {uri}"
        );
    }
}

#[test]
fn posix_decoding_rejects_what_it_cannot_name() {
    for (uri, path) in POSIX_DECODE_ONLY {
        assert_eq!(
            decode_uri(uri, PathStyle::Posix).as_deref(),
            *path,
            "decode {uri}"
        );
    }
}

#[test]
fn native_round_trip() {
    let path = if cfg!(windows) {
        PathBuf::from(r"C:\work\a b.rs")
    } else {
        PathBuf::from("/work/a b.rs")
    };
    assert_eq!(uri_to_path(&path_to_uri(&path)), Some(path));
}
