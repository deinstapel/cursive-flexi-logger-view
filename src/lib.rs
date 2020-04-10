//! # A `FlexiLoggerView` for cursive
//!
//! This crate provides a new debug view for
//! [gyscos/cursive](https://github.com/gyscos/cursive) using the
//! [emabee/flexi_logger](https://github.com/emabee/flexi_logger) crate. This
//! enables the `FlexiLoggerView` to respect the `RUST_LOG` environment variable
//! as well as the `flexi_logger` configuration file. Have a look at the `demo`
//! below to see how it looks.
//!
//! ## Using the `FlexiLoggerView`
//!
//! To create a `FlexiLoggerView` you first have to register the
//! `cursive_flexi_logger` as a `LogTarget` in `flexi_logger`. After the
//! `flexi_logger` has started, you may create a `FlexiLoggerView` instance and
//! add it to cursive.
//!
//! ```rust
//! use cursive::Cursive;
//! use cursive_flexi_logger_view::FlexiLoggerView;
//! use flexi_logger::{Logger, LogTarget};
//!
//! fn main() {
//!     // we need to initialize cursive first, as the cursive-flexi-logger
//!     // needs a cursive callback sink to notify cursive about screen refreshs
//!     // when a new log message arrives
//!     let mut siv = Cursive::default();
//!
//!     Logger::with_env_or_str("trace")
//!         .log_target(LogTarget::FileAndWriter(
//!             cursive_flexi_logger_view::cursive_flexi_logger(&siv),
//!         ))
//!         .directory("logs")
//!         .suppress_timestamp()
//!         .format(flexi_logger::colored_with_thread)
//!         .start()
//!         .expect("failed to initialize logger!");
//!
//!     siv.add_layer(FlexiLoggerView::scrollable()); // omit `scrollable` to remove scrollbars
//!
//!     log::info!("test log message");
//!     // siv.run();
//! }
//! ```
//!
//! Look into the `FlexiLoggerView` documentation for a detailed explanation.

use arraydeque::{ArrayDeque, Wrapping};
use cursive::theme::{BaseColor, Color};
use cursive::utils::markup::StyledString;
use cursive::view::{Nameable, ScrollStrategy, Scrollable, View};
use cursive::views::{Dialog, ScrollView};
use cursive::{CbSink, Cursive, Printer, Vec2};
use flexi_logger::{writers::LogWriter, DeferredNow, Level, Record};

use std::sync::{Arc, Mutex};
use std::thread;

type LogBuffer = ArrayDeque<[StyledString; 2048], Wrapping>;

static FLEXI_DEBUG_VIEW_NAME: &str = "_flexi_debug_view";

lazy_static::lazy_static! {
    static ref LOGS: Arc<Mutex<LogBuffer>> = Arc::new(Mutex::new(LogBuffer::new()));
}

/// The `FlexiLoggerView` displays log messages from the `cursive_flexi_logger` log target.
/// It is safe to create multiple instances of this struct.
///
/// # Create a plain `FlexiLoggerView`
///
/// ```rust
/// use cursive::Cursive;
/// use cursive_flexi_logger_view::FlexiLoggerView;
/// use flexi_logger::{Logger, LogTarget};
///
/// fn main() {
///     // we need to initialize cursive first, as the cursive-flexi-logger
///     // needs a cursive callback sink to notify cursive about screen refreshs
///     // when a new log message arrives
///     let mut siv = Cursive::default();
///
///     Logger::with_env_or_str("trace")
///         .log_target(LogTarget::FileAndWriter(
///             cursive_flexi_logger_view::cursive_flexi_logger(&siv),
///         ))
///         .directory("logs")
///         .suppress_timestamp()
///         .format(flexi_logger::colored_with_thread)
///         .start()
///         .expect("failed to initialize logger!");
///
///     siv.add_layer(FlexiLoggerView); // add a plain flexi-logger view
///
///     log::info!("test log message");
///     // siv.run();
/// }
/// ```
///
/// # Create a scrollable `FlexiLoggerView`
///
/// ```rust
/// use cursive::Cursive;
/// use cursive_flexi_logger_view::FlexiLoggerView;
/// use flexi_logger::{Logger, LogTarget};
///
/// fn main() {
///     // we need to initialize cursive first, as the cursive-flexi-logger
///     // needs a cursive callback sink to notify cursive about screen refreshs
///     // when a new log message arrives
///     let mut siv = Cursive::default();
///
///     Logger::with_env_or_str("trace")
///         .log_target(LogTarget::FileAndWriter(
///             cursive_flexi_logger_view::cursive_flexi_logger(&siv),
///         ))
///         .directory("logs")
///         .suppress_timestamp()
///         .format(flexi_logger::colored_with_thread)
///         .start()
///         .expect("failed to initialize logger!");
///
///     siv.add_layer(FlexiLoggerView::scrollable()); // add a scrollable flexi-logger view
///
///     log::info!("test log message");
///     // siv.run();
/// }
/// ```
pub struct FlexiLoggerView;

impl FlexiLoggerView {
    /// Create a new `FlexiLoggerView` which is wrapped in a `ScrollView`.
    pub fn scrollable() -> ScrollView<Self> {
        FlexiLoggerView
            .scrollable()
            .scroll_x(true)
            .scroll_y(true)
            .scroll_strategy(ScrollStrategy::StickToBottom)
    }
}

impl View for FlexiLoggerView {
    fn draw(&self, printer: &Printer<'_, '_>) {
        let logs = LOGS.lock().unwrap();

        // Only print the last logs, so skip what doesn't fit
        let skipped = logs.len().saturating_sub(printer.size.y);

        for (i, msg) in logs.iter().skip(skipped).enumerate() {
            printer.print_styled((0, i), msg.into());
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let logs = LOGS.lock().unwrap();

        // The longest line sets the width
        let w = logs
            .iter()
            .map(|msg| msg.width())
            .max()
            .unwrap_or(1);
        let h = logs.len();
        let w = std::cmp::max(w, constraint.x);
        let h = std::cmp::max(h, constraint.y);

        Vec2::new(w, h)
    }
}

/// The `flexi_logger` `LogWriter` implementation for the `FlexiLoggerView`.
///
/// Use the `cursive_flexi_logger` function to create an instance of this struct.
pub struct CursiveLogWriter {
    sink: CbSink,
}

/// Creates a new `LogWriter` instance for the `FlexiLoggerView`. Use this to
/// register a cursive log writer in `flexi_logger`.
///
/// Although, it is safe to create multiple cursive log writers, it may not be
/// what you want. Each instance of a cursive log writer replicates the log
/// messages in to `FlexiLoggerView`. When registering multiple cursive log
/// writer instances, a single log messages will be duplicated by each log
/// writer.
///
/// # Registering the cursive log writer in `flexi_logger`
///
/// ```rust
/// use cursive::Cursive;
/// use flexi_logger::{Logger, LogTarget};
///
/// fn main() {
///     // we need to initialize cursive first, as the cursive-flexi-logger
///     // needs a cursive callback sink to notify cursive about screen refreshs
///     // when a new log message arrives
///     let mut siv = Cursive::default();
///
///     Logger::with_env_or_str("trace")
///         .log_target(LogTarget::FileAndWriter(
///             cursive_flexi_logger_view::cursive_flexi_logger(&siv), // register log writer
///         ))
///         .directory("logs")
///         .suppress_timestamp()
///         .format(flexi_logger::colored_with_thread)
///         .start()
///         .expect("failed to initialize logger!");
/// }
/// ```
pub fn cursive_flexi_logger(siv: &Cursive) -> Box<CursiveLogWriter> {
    Box::new(CursiveLogWriter {
        sink: siv.cb_sink().clone(),
    })
}

impl LogWriter for CursiveLogWriter {
    fn write(&self, now: &mut DeferredNow, record: &Record) -> std::io::Result<()> {
        let color = Color::Dark(match record.level() {
            Level::Trace => BaseColor::Green,
            Level::Debug => BaseColor::Cyan,
            Level::Info => BaseColor::Blue,
            Level::Warn => BaseColor::Yellow,
            Level::Error => BaseColor::Red,
        });

        let mut line = StyledString::new();
        line.append_styled(format!("{}", now.now().format("%T%.3f")), color);
        line.append_plain(format!(
            " [{}] ",
            thread::current().name().unwrap_or("(unnamed)"),
        ));
        line.append_styled(format!("{}", record.level()), color);
        line.append_plain(format!(
            " <{}:{}> ",
            record.file().unwrap_or("(unnamed)"),
            record.line().unwrap_or(0),
        ));
        line.append_styled(format!("{}", &record.args()), color);

        LOGS.lock().unwrap().push_back(line);
        self.sink.send(Box::new(|_siv| {}))
            .map_err(|_| std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "cursive callback sink is closed!",
            ))
    }

    fn flush(&self) -> std::io::Result<()> {
        // we are not buffering
        Ok(())
    }

    fn max_log_level(&self) -> log::LevelFilter {
        log::LevelFilter::max()
    }
}

/// Show the flexi debug console.
///
/// This analog for `Cursive::show_debug_console`.
pub fn show_flexi_debug_console(siv: &mut Cursive) {
    siv.add_layer(
        Dialog::around(FlexiLoggerView::scrollable().with_name(FLEXI_DEBUG_VIEW_NAME))
            .title("Debug console"),
    );
}

/// Show the flexi debug console, or hide it if it's already visible.
///
/// This analog for `Cursive::toggle_debug_console`.
///
/// # Enable toggleable flexi debug view
///
/// ```rust
/// use cursive::Cursive;
/// use cursive_flexi_logger_view::toggle_flexi_debug_console;
/// use flexi_logger::{Logger, LogTarget};
///
/// fn main() {
///     // we need to initialize cursive first, as the cursive-flexi-logger
///     // needs a cursive callback sink to notify cursive about screen refreshs
///     // when a new log message arrives
///     let mut siv = Cursive::default();
///
///     Logger::with_env_or_str("trace")
///         .log_target(LogTarget::FileAndWriter(
///             cursive_flexi_logger_view::cursive_flexi_logger(&siv),
///         ))
///         .directory("logs")
///         .suppress_timestamp()
///         .format(flexi_logger::colored_with_thread)
///         .start()
///         .expect("failed to initialize logger!");
///
///     siv.add_global_callback('~', toggle_flexi_debug_console);  // Enable toggleable flexi debug view
///
///     // siv.run();
/// }
/// ```
pub fn toggle_flexi_debug_console(siv: &mut Cursive) {
    if let Some(pos) = siv.screen_mut().find_layer_from_name(FLEXI_DEBUG_VIEW_NAME) {
        siv.screen_mut().remove_layer(pos);
    } else {
        show_flexi_debug_console(siv);
    }
}
