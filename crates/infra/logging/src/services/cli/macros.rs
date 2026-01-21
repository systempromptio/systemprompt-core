#[macro_export]
macro_rules! cli_success {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::success(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_warning {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::warning(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_error {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::error(&format!($($arg)*))
    };
}

#[macro_export]
macro_rules! cli_info {
    ($($arg:tt)*) => {
        $crate::services::cli::CliService::info(&format!($($arg)*))
    };
}
