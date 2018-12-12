mod race;
mod tree;
mod tui;

use crate::race::Race;

use std::process::exit;

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
