//! Tests for the desktop file-loading path of [`miniquad::fs`].
//!
//! On desktops `load_file` invokes the callback synchronously, which makes the
//! behavior testable without a window or event loop.

use std::sync::mpsc;
use std::time::Duration;

fn load(path: &str) -> miniquad::fs::Response {
    let (tx, rx) = mpsc::channel();
    miniquad::fs::load_file(path, move |response| {
        tx.send(response).expect("receiver dropped");
    });
    rx.recv_timeout(Duration::from_secs(5))
        .expect("desktop load_file must call back synchronously")
}

#[test]
fn loads_existing_file() {
    let mut path = std::env::temp_dir();
    path.push("miniquad_fs_test_existing.txt");
    std::fs::write(&path, b"miniquad file contents").unwrap();

    let response = load(path.to_str().unwrap());

    assert_eq!(response.unwrap(), b"miniquad file contents");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn reports_missing_file_as_io_error() {
    let response = load("/definitely/not/a/real/path/miniquad_test");

    let err = response.expect_err("missing file must produce an error");
    match err {
        miniquad::fs::Error::IOError(_) => {}
        other => panic!("expected IOError, got {other:?}"),
    }
}

#[test]
fn fs_error_display() {
    let err = miniquad::fs::Error::DownloadFailed;
    assert_eq!(err.to_string(), "Download failed");

    let err = miniquad::fs::Error::AndroidAssetLoadingError;
    assert_eq!(err.to_string(), "[android] Failed to load asset");

    let io: miniquad::fs::Error = std::io::Error::new(std::io::ErrorKind::NotFound, "nope").into();
    let msg = io.to_string();
    assert!(msg.starts_with("I/O error: "), "unexpected: {msg}");
}

#[test]
fn date_now_is_a_plausible_epoch_timestamp() {
    let now = miniquad::date::now();
    // Between 2026-01-01 and 2100-01-01; guards against, say, returning
    // milliseconds instead of seconds or a negative/zero placeholder.
    assert!(now > 1_767_225_600.0, "now too small: {now}");
    assert!(now < 4_102_444_800.0, "now too large: {now}");
}
