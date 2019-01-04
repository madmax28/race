mod process;
mod race;
mod tui;
mod util;

use crate::race::Race;

use std::process::exit;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

fn usage() -> ! {
    println!("Usage: race <program> [args..]");
    exit(1);
}

fn main() {
    let program = std::env::args().nth(1).unwrap_or_else(|| usage());
    let args: Vec<String> = std::env::args().skip(1).collect();

    let child = race::fork_child(&program, &args);
    let mut race = Race::new(child);
    race.trace();
    race.dump_tree();
}
