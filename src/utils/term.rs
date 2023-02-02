use atty;

#[macro_export]
macro_rules! println_err {
    ($($arg:tt)*) => (
        if $crate::utils::term::is_term() {
            error!($($arg)*);
            println!("{}", ansi_term::Color::Red.bold().paint(format!($($arg)*)))
        } else {
            println!($($arg)*)
        }
    )
}

#[macro_export]
macro_rules! println_succ {
    ($($arg:tt)*) => (
        if $crate::utils::term::is_term() {
            trace!($($arg)*);
            println!("{}", ansi_term::Color::Green.bold().paint(format!($($arg)*)))
        } else {
            println!($($arg)*)
        }
    )
}

#[macro_export]
macro_rules! println_warn {
    ($($arg:tt)*) => (
        if $crate::utils::term::is_term() {
            println!("{}", ansi_term::Color::Yellow.bold().paint(format!($($arg)*)))
        } else {
            trace!($($arg)*);
            println!($($arg)*)
        }
    )
}

#[macro_export]
macro_rules! println_acc {
    ($($arg:tt)*) => (
       if $crate::utils::term::is_term() {
            trace!($($arg)*);
           println!("{}", ansi_term::Style::new().bold().paint(format!($($arg)*)))
       } else {
           println!($($arg)*)
       }
    )
}

pub fn is_term() -> bool {
    atty::is(atty::Stream::Stdout)
}
