pub mod env_vars {
    pub const SYS_BASEPATH: &str = "SYS_BASE_PATH";
    pub const SERVICE_BASEPATH: &str = "SERVICE_BASE_PATH";
    // relative path starting from app / service home folder
    pub const CFG_FILEPATH: &str = "CONFIG_FILE_PATH";
    pub const EXPECTED_LABELS: [&str; 3] = [SYS_BASEPATH, SERVICE_BASEPATH, CFG_FILEPATH];
}

pub(crate) const REGEX_EMAIL_RFC5322: &str = r#"(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*|"(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21\x23-\x5b\x5d-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])*")@(?:(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?|\[(?:(?:(2(5[0-5]|[0-4][0-9])|1[0-9][0-9]|[1-9]?[0-9]))\.){3}(?:(2(5[0-5]|[0-4][0-9])|1[0-9][0-9]|[1-9]?[0-9])|[a-z0-9-]*[a-z0-9]:(?:[\x01-\x08\x0b\x0c\x0e-\x1f\x21-\x5a\x53-\x7f]|\\[\x01-\x09\x0b\x0c\x0e-\x7f])+)\])"#;

pub mod logging {
    use serde::Deserialize;

    #[allow(clippy::upper_case_acronyms)]
    #[derive(Deserialize)]
    pub enum Level {
        TRACE,
        DEBUG,
        INFO,
        WARNING,
        ERROR,
        FATAL,
    }

    #[allow(clippy::upper_case_acronyms)]
    #[derive(Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Destination {
        CONSOLE,
        LOCALFS,
    } // TODO, Fluentd
}
