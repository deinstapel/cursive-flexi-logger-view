use cursive::{Cursive, CursiveExt, Vec2};
use cursive::view::Boxable;
use cursive::views::Dialog;
use cursive_flexi_logger_view::FlexiLoggerView;

use std::time::Duration;

fn main() {
    // we need to initialize cursive first, as the cursive-flexi-logger
    // needs a cursive callback sink to notify cursive about screen refreshs
    // when a new log message arrives
    let mut siv = Cursive::default();

    flexi_logger::Logger::with_env_or_str("trace")
        .log_target(flexi_logger::LogTarget::FileAndWriter(
            cursive_flexi_logger_view::cursive_flexi_logger(&siv),
        ))
        .directory("logs")
        .suppress_timestamp()
        .format(flexi_logger::colored_with_thread)
        .start()
        .expect("failed to initialize logger!");

    siv.add_layer(
        Dialog::around(FlexiLoggerView::scrollable())
            .title("Flexi-Logger View")
            .button("Quit", |siv| siv.quit())
            .fixed_size(Vec2::new(72, 10))
    );

    log::info!("started simple example");

    let sink = siv.cb_sink().clone();
    std::thread::Builder::new().name("worker".to_string()).spawn(move || {
        log::trace!("A trace log message");
        std::thread::sleep(Duration::from_secs(1));

        log::debug!("A debug log message");
        std::thread::sleep(Duration::from_secs(1));

        log::info!("An info log message");
        std::thread::sleep(Duration::from_secs(1));

        log::debug!("Really detailed debug information\nfoo: 5\nbar: 42");
        std::thread::sleep(Duration::from_secs(1));

        log::warn!("A warning log message");
        std::thread::sleep(Duration::from_secs(1));

        log::error!("An error log message");
        std::thread::sleep(Duration::from_secs(1));

        sink.send(Box::new(|siv| siv.quit())).expect("failed to quit");
    }).expect("worker thread panicked!");

    siv.run();
}
