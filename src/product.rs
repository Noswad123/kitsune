pub(crate) const NAME: &str = "Kitsune";
pub(crate) const CLI_NAME: &str = "kitsune";
pub(crate) const SHORT_CLI_NAME: &str = "kit";
pub(crate) const DESCRIPTION: &str = "terminal workspace manager for AI coding agents";
pub(crate) const CONFIG_DIR_NAME: &str = "kitsune";
pub(crate) const DEV_CONFIG_DIR_NAME: &str = "kitsune-dev";
pub(crate) const LOG_FILE_NAME: &str = "kitsune.log";
pub(crate) const CLIENT_LOG_FILE_NAME: &str = "kitsune-client.log";
pub(crate) const SERVER_LOG_FILE_NAME: &str = "kitsune-server.log";
pub(crate) const LOG_ENV_VAR: &str = "KITSUNE_LOG";
pub(crate) const API_SOCKET_FILE_NAME: &str = "kitsune.sock";
pub(crate) const CLIENT_SOCKET_FILE_NAME: &str = "kitsune-client.sock";
pub(crate) const CONFIG_PATH_ENV_VAR: &str = "KITSUNE_CONFIG_PATH";
pub(crate) const SOCKET_PATH_ENV_VAR: &str = "KITSUNE_SOCKET_PATH";
pub(crate) const CLIENT_SOCKET_PATH_ENV_VAR: &str = "KITSUNE_CLIENT_SOCKET_PATH";
pub(crate) const SESSION_ENV_VAR: &str = "KITSUNE_SESSION";
pub(crate) const RUNTIME_ENV_VAR: &str = "KITSUNE_ENV";
pub(crate) const RUNTIME_ENV_VALUE: &str = "1";

pub(crate) fn command(command: &str) -> String {
    format!("{CLI_NAME} {command}")
}
