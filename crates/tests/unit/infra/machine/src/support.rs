use std::path::PathBuf;

/// An absolute program path on the system the tests run on, with a blank in it.
pub(crate) fn program() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\Program Files\Example App\example.exe")
    } else {
        PathBuf::from("/opt/example app/example")
    }
}

pub(crate) fn program_text() -> String {
    program().to_str().expect("utf-8 program path").to_owned()
}
