use prebindgen_proc_macro::prebindgen;

/// Initialize Android-style logging (logcat on Android, standard
/// output on non-Android systems) with an explicit filter.
///
/// If the logger was already initialized, this call is a no-op.
///
/// See <https://docs.rs/env_logger/latest/env_logger/index.html> for
/// accepted filter format.
#[prebindgen]
pub fn init_android_logs(filter: &str) {
    android_logd_logger::builder()
        .parse_filters(filter)
        .tag_target_strip()
        .prepend_module(true)
        .try_init()
        .ok();
}

/// Try to initialize zenoh logging from environment variables.
#[prebindgen]
pub fn try_init_zenoh_logs_from_env() {
    zenoh::try_init_log_from_env();
}

/// Initialize zenoh logging from environment variables, falling back to
/// `fallback_filter` when the environment is unset.
#[prebindgen]
pub fn init_zenoh_logs_from_env_or(fallback_filter: &str) {
    zenoh::init_log_from_env_or(fallback_filter);
}
