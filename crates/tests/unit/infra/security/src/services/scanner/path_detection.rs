//! Tests for ScannerDetector path detection (extensions and directories)

use systemprompt_security::ScannerDetector;

// ============================================================================
// Scanner Path Detection - Extension Tests
// ============================================================================

#[test]
fn test_is_scanner_path_php_extension() {
    assert!(ScannerDetector::is_scanner_path("/admin.php"));
    assert!(ScannerDetector::is_scanner_path("/test.PHP"));
    assert!(ScannerDetector::is_scanner_path("/path/to/file.php"));
}

#[test]
fn test_is_scanner_path_env_extension() {
    assert!(ScannerDetector::is_scanner_path("/config.env"));
    assert!(ScannerDetector::is_scanner_path("/path/settings.env"));
    assert!(ScannerDetector::is_scanner_path("/app/prod.ENV"));
}

#[test]
fn test_is_scanner_path_git_extension() {
    assert!(ScannerDetector::is_scanner_path("/config.git"));
    assert!(ScannerDetector::is_scanner_path("/repo/file.git"));
}

#[test]
fn test_is_scanner_path_sql_extension() {
    assert!(ScannerDetector::is_scanner_path("/backup.sql"));
    assert!(ScannerDetector::is_scanner_path("/dump.SQL"));
}

#[test]
fn test_is_scanner_path_backup_extensions() {
    assert!(ScannerDetector::is_scanner_path("/config.bak"));
    assert!(ScannerDetector::is_scanner_path("/old_file.old"));
    assert!(ScannerDetector::is_scanner_path("/archive.zip"));
    assert!(ScannerDetector::is_scanner_path("/backup.gz"));
}

#[test]
fn test_is_scanner_path_db_extension() {
    assert!(ScannerDetector::is_scanner_path("/database.db"));
    assert!(ScannerDetector::is_scanner_path("/app.DB"));
}

#[test]
fn test_is_scanner_path_config_extension() {
    assert!(ScannerDetector::is_scanner_path("/app.config"));
    assert!(ScannerDetector::is_scanner_path("/settings.CONFIG"));
}

#[test]
fn test_is_scanner_path_cgi_extension() {
    assert!(ScannerDetector::is_scanner_path("/script.cgi"));
    assert!(ScannerDetector::is_scanner_path("/handler.CGI"));
}

#[test]
fn test_is_scanner_path_htm_extension() {
    assert!(ScannerDetector::is_scanner_path("/page.htm"));
    assert!(ScannerDetector::is_scanner_path("/index.HTM"));
}

// ============================================================================
// Scanner Path Detection - Directory Tests
// ============================================================================

#[test]
fn test_is_scanner_path_admin_directory() {
    assert!(ScannerDetector::is_scanner_path("/admin"));
    assert!(ScannerDetector::is_scanner_path("/admin/"));
    assert!(ScannerDetector::is_scanner_path("/admin/login"));
    assert!(ScannerDetector::is_scanner_path("/ADMIN"));
}

#[test]
fn test_is_scanner_path_wp_admin() {
    assert!(ScannerDetector::is_scanner_path("/wp-admin"));
    assert!(ScannerDetector::is_scanner_path("/wp-admin/"));
    assert!(ScannerDetector::is_scanner_path("/wp-admin/admin.php"));
}

#[test]
fn test_is_scanner_path_wp_content() {
    assert!(ScannerDetector::is_scanner_path("/wp-content"));
    assert!(ScannerDetector::is_scanner_path("/wp-content/uploads"));
    assert!(ScannerDetector::is_scanner_path("/wp-content/plugins"));
}

#[test]
fn test_is_scanner_path_uploads() {
    assert!(ScannerDetector::is_scanner_path("/uploads"));
    assert!(ScannerDetector::is_scanner_path("/uploads/"));
    assert!(ScannerDetector::is_scanner_path("/uploads/images"));
}

#[test]
fn test_is_scanner_path_cgi_bin() {
    assert!(ScannerDetector::is_scanner_path("/cgi-bin"));
    assert!(ScannerDetector::is_scanner_path("/cgi-bin/"));
    assert!(ScannerDetector::is_scanner_path("/cgi-bin/script.pl"));
}

#[test]
fn test_is_scanner_path_phpmyadmin() {
    assert!(ScannerDetector::is_scanner_path("/phpmyadmin"));
    assert!(ScannerDetector::is_scanner_path("/phpMyAdmin"));
    assert!(ScannerDetector::is_scanner_path("/PHPMYADMIN"));
}

#[test]
fn test_is_scanner_path_xmlrpc() {
    assert!(ScannerDetector::is_scanner_path("/xmlrpc"));
    assert!(ScannerDetector::is_scanner_path("/xmlrpc.php"));
}

#[test]
fn test_is_scanner_path_shell_php() {
    assert!(ScannerDetector::is_scanner_path("/shell.php"));
    assert!(ScannerDetector::is_scanner_path("/path/shell.php"));
}

#[test]
fn test_is_scanner_path_c99() {
    assert!(ScannerDetector::is_scanner_path("/c99.php"));
    assert!(ScannerDetector::is_scanner_path("/uploads/c99.php"));
}

#[test]
fn test_is_scanner_path_eval_stdin() {
    assert!(ScannerDetector::is_scanner_path("/eval-stdin.php"));
}

#[test]
fn test_is_scanner_path_setup_cgi() {
    assert!(ScannerDetector::is_scanner_path("/setup.cgi"));
}

#[test]
fn test_is_scanner_path_manager_html() {
    assert!(ScannerDetector::is_scanner_path("/manager/html"));
    assert!(ScannerDetector::is_scanner_path("/manager/html/"));
}

#[test]
fn test_is_scanner_path_config_directory() {
    assert!(ScannerDetector::is_scanner_path("/config/"));
    assert!(ScannerDetector::is_scanner_path("/config/settings"));
}

#[test]
fn test_is_scanner_path_identity() {
    assert!(ScannerDetector::is_scanner_path("/identity"));
    assert!(ScannerDetector::is_scanner_path("/identity/"));
}

#[test]
fn test_is_scanner_path_login_htm() {
    assert!(ScannerDetector::is_scanner_path("/login.htm"));
}

// ============================================================================
// Scanner Path Detection - Legitimate Paths
// ============================================================================

#[test]
fn test_is_scanner_path_legitimate_paths() {
    assert!(!ScannerDetector::is_scanner_path("/"));
    assert!(!ScannerDetector::is_scanner_path("/api/users"));
    assert!(!ScannerDetector::is_scanner_path("/api/v1/data"));
    assert!(!ScannerDetector::is_scanner_path("/health"));
    assert!(!ScannerDetector::is_scanner_path("/status"));
    assert!(!ScannerDetector::is_scanner_path("/login"));
    assert!(!ScannerDetector::is_scanner_path("/dashboard"));
}

#[test]
fn test_is_scanner_path_static_assets() {
    assert!(!ScannerDetector::is_scanner_path("/static/app.js"));
    assert!(!ScannerDetector::is_scanner_path("/assets/style.css"));
    assert!(!ScannerDetector::is_scanner_path("/images/logo.png"));
    assert!(!ScannerDetector::is_scanner_path("/fonts/roboto.woff2"));
}
