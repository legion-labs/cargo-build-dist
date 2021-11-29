use std::{fmt::Display, io::Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub const ACTION_STEP_COLOR: Color = Color::Green;
pub const IGNORE_STEP_COLOR: Color = Color::Yellow;

pub fn print_step(color: Color, action: &str, description: impl Display) {
    if atty::is(atty::Stream::Stdout) {
        let mut stdout = StandardStream::stdout(ColorChoice::Always);
        stdout
            .set_color(
                ColorSpec::new()
                    .set_fg(Some(color))
                    .set_intense(true)
                    .set_bold(true),
            )
            .unwrap();
        write!(
            &mut stdout,
            "{}{}",
            (0..(12 - action.len())).map(|_| " ").collect::<String>(),
            action
        )
        .unwrap();
        stdout.reset().unwrap();
        writeln!(&mut stdout, " {}", description).unwrap();
    } else {
        println!(
            "{}{} {}",
            (0..(12 - action.len())).map(|_| " ").collect::<String>(),
            action,
            description
        );
    }
}

#[macro_export]
macro_rules! action_step {
    ($action:expr, $description:expr $(,)?) => {
        $crate::term::print_step($crate::ACTION_STEP_COLOR, $action, $description)
    };
    ($action:expr, $fmt:expr, $($arg:tt)*) => {
        action_step!($action, format!($fmt, $($arg)*))
    };
}

#[macro_export]
macro_rules! ignore_step {
    ($action:expr, $description:expr $(,)?) => {
        $crate::term::print_step($crate::IGNORE_STEP_COLOR, $action, $description)
    };
    ($action:expr, $fmt:expr, $($arg:tt)*) => {
        ignore_step!($action, format!($fmt, $($arg)*))
    };
}
