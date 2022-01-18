use log::*;
use std::error::Error;
use std::fmt;

pub type BoxResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct MyError {
    msg: &'static str,
}

impl fmt::Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MyError is {}", self.msg)
    }
}

impl serde::ser::StdError for MyError {}

pub fn strerror<T>(msg: &'static str) -> BoxResult<T> {
    Err(Box::new(MyError { msg }))
}

// ===================== Logging Set Up =====================
pub fn set_up_logging(log_filter: log::LevelFilter, opt_fname: Option<String>) -> BoxResult<()> {
    use fern::colors::*;
    // configure colors for the whole line
    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::Green)
        .debug(Color::Blue)
        // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);

    // configure colors for the name of the level.
    // since almost all of them are the same as the color for the whole line, we
    // just clone `colors_line` and overwrite our changes
    let colors_level = colors_line.info(Color::Green);
    // here we set up our fern Dispatch
    let mut logger = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}{date} {level} {target} {color_line}{message}\x1B[0m",
                color_line = format_args!(
                    "\x1B[{}m",
                    colors_line.get_color(&record.level()).to_fg_str()
                ),
                date = chrono::Local::now().format("%H:%M:%S"),
                target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        // set the default log level. to filter out verbose log messages from dependencies, set
        // this to Warn and overwrite the log level for your crate.
        .level(log_filter)
        // change log levels for individual modules. Note: This looks for the record's target
        // field which defaults to the module path but can be overwritten with the `target`
        // parameter:
        // `info!(target="special_target", "This log message is about special_target");`
        //.level_for("pretty_colored", log::LevelFilter::Trace)
        // output to stdout
        .chain(std::io::stdout());

    if let Some(fname) = opt_fname {
        logger = logger.chain(fern::log_file(fname)?);
    }

    logger.apply().unwrap();

    debug!("finished setting up logging! yay!");
    Ok(())
}
