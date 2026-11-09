#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;

// ANSI escape sequences used to colorize the console output.
// See <https://en.wikipedia.org/wiki/ANSI_escape_code#SGR_(Select_Graphic_Rendition)_parameters>
const RESET: &str = "\x1b[0m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const BOLD_BLUE: &str = "\x1b[1;34m";

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    // Keep the plain text `[WithColor]: Hello, Arceos!` intact so that the
    // output is still greppable after stripping the escape sequences.
    println!("{BOLD_GREEN}[WithColor]{RESET}: {BOLD_BLUE}Hello, Arceos!{RESET}");
}
