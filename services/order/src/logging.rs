use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::io::stdout;
use std::path::Path;

use tracing::dispatcher::Dispatch;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::fmt::{
    writer::{MakeWriterExt, WithMaxLevel}, // BoxMakeWriter is for type-erasion of low-level writer, it does not support clone
    // ArcWriter implementation is NOT completed, would be removed in future version.
    Layer as TraceLayer,
};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::{self, Registry};

use crate::constant::logging::{Destination as DstOption, Level as AppLogLevelInner};
use crate::{AppBasepathCfg, AppLogAlias, AppLogHandlerCfg, AppLoggerCfg, AppLoggingCfg};

pub type AppLogLevel = AppLogLevelInner;
type AppLogHandler = (WithMaxLevel<NonBlocking>, WorkerGuard);
type AppLogger = Dispatch;

pub struct AppLogContext {
    handlers: HashMap<AppLogAlias, AppLogHandler>,
    loggers: HashMap<AppLogAlias, AppLogger, RandomState>,
}

// this macro has to be exposed since top-level binary executable (e.g. web)
// will invoke this macro indirectly
#[macro_export]
macro_rules! to_3rdparty_level {
    ($lvlin:expr) => {
        match $lvlin {
            $crate::logging::AppLogLevel::FATAL | $crate::logging::AppLogLevel::ERROR => {
                tracing::Level::ERROR
            }
            $crate::logging::AppLogLevel::WARNING => tracing::Level::WARN,
            $crate::logging::AppLogLevel::INFO => tracing::Level::INFO,
            $crate::logging::AppLogLevel::DEBUG => tracing::Level::DEBUG,
            $crate::logging::AppLogLevel::TRACE => tracing::Level::TRACE,
        } // in `tracing` ecosystem, level comparison is like
          // TRACE > DEBUG > INFO > WARN > ERROR
    };
}

fn _gen_localfile_writer(basepath: &String, cfg: &AppLogHandlerCfg) -> (NonBlocking, WorkerGuard) {
    if let Some(rpath) = cfg.path.as_ref() {
        let mut fullpath = basepath.clone();
        if !basepath.ends_with("/") && !rpath.starts_with("/") {
            fullpath = fullpath + "/";
        }
        fullpath = fullpath + &rpath;
        let p = Path::new(&fullpath);
        let (dir, fname_prefix) = (p.parent().unwrap(), p.file_name().unwrap());
        let wr_dst = RollingFileAppender::new(Rotation::NEVER, dir, fname_prefix);
        tracing_appender::non_blocking(wr_dst)
    } else {
        panic!(
            "File:{}, Line:{}, configure path has to be always present",
            file!(),
            line!()
        );
    }
}

fn _gen_console_writer(_: &AppLogHandlerCfg) -> (NonBlocking, WorkerGuard) {
    let wr_dst = stdout();
    tracing_appender::non_blocking(wr_dst)
} // Note tracing spawns new thread dedicating to each non-blocking writer,
  // the context-switching rule depends on underlying OS platform.

fn _init_handler(basepath: &AppBasepathCfg, cfg: &AppLogHandlerCfg) -> AppLogHandler {
    let lvl = to_3rdparty_level!(&cfg.min_level);
    let (io_wr, guard) = match &cfg.destination {
        DstOption::CONSOLE => _gen_console_writer(cfg),
        DstOption::LOCALFS => _gen_localfile_writer(&basepath.system, cfg),
    }; // callers MUST always keep the guard along with writer, for successfully flushing
       // log messages to I/O
    let io_wr = io_wr.with_max_level(lvl);
    (io_wr, guard)
}

fn _init_logger(cfg: &AppLoggerCfg, hdlrs: &HashMap<AppLogAlias, AppLogHandler>) -> AppLogger {
    let iter = cfg.handlers.iter().map(|alias| {
        let (io_writer, _) = hdlrs.get(alias).unwrap();
        TraceLayer::new()
            .with_writer(io_writer.clone())
            .with_file(false) // to prevent full path exposed
            .with_line_number(true)
            .with_thread_ids(true)
            .with_level(true)
    });
    let layers = Vec::from_iter(iter);
    let subscriber = Registry::default().with(layers);
    //let alias = cfg.handlers.iter().next().unwrap();
    Dispatch::new(subscriber)
} // end of _init_logger

impl AppLogContext {
    pub fn new(basepath: &AppBasepathCfg, cfg: &AppLoggingCfg) -> Self {
        let iter = cfg
            .handlers
            .iter()
            .map(|item| (item.alias.clone(), _init_handler(basepath, item)));
        let hdlrs = HashMap::from_iter(iter);
        let iter2 = cfg
            .loggers
            .iter()
            .map(|item| (item.alias.clone(), _init_logger(item, &hdlrs)));
        let logger_map: HashMap<AppLogAlias, Dispatch, RandomState> = HashMap::from_iter(iter2);
        Self {
            handlers: hdlrs,
            loggers: logger_map,
        }
    }

    pub fn get_assigner(&self, key: &str) -> Option<&Dispatch> {
        self.loggers.get(&key.to_string())
    }
} // end of impl AppLogContext

//let myspan = tracing::span!(Level::TRACE, "test-trace-123"); // span not necessary
//let _entered = myspan.enter();

#[macro_export]
macro_rules! app_log_event {
    ( $ctx:ident, $lvl:expr, $($arg:tt)+ ) => {{
        const MOD_PATH:&str = module_path!();
        if let Some(assigner) = $ctx.get_assigner(MOD_PATH) {
            const LVL_INNER: tracing::Level = $crate::logging::to_3rdparty_level!($lvl);
            tracing::dispatcher::with_default(assigner, || {
                tracing::event!(LVL_INNER, $($arg)+);
            });
        } else {
            println!("[WARN] log dispatcher not found at the module path: {}", MOD_PATH);
            println!($($arg)+);
        }
    }};
}

pub use app_log_event;
pub use to_3rdparty_level;
