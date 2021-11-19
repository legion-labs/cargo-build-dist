use std::{fmt::Display, io::Write};
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

pub fn print_step(action: &str, description: impl Display) {
    let mut stdout = StandardStream::stderr(ColorChoice::Always);
    stdout
        .set_color(
            ColorSpec::new()
                .set_fg(Some(Color::Green))
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
    write!(&mut stdout, " {}\n", description).unwrap();
}

#[macro_export]
macro_rules! step {
    ($action:expr, $description:expr $(,)?) => {
        $crate::term::print_step($action, $description)
    };
    ($action:expr, $fmt:expr, $($arg:tt)*) => {
        step!($action, format!($fmt, $($arg)*))
    };
}
