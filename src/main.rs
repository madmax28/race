mod args;
mod process;
mod race;
mod tui;
mod util;

use crate::tui::{term, tv};

use std::fs;
use std::io;
use std::io::Write;

type Result<T> = std::result::Result<T, failure::Error>;

fn main() {
    let args = args::parse_args();

    let program = args.value_of("PROGRAM").unwrap();
    let program_args: Vec<_> = args
        .values_of("ARGS")
        .and_then(|vs| Some(vs.collect()))
        .unwrap_or_default();

    let mut race = match race::Race::fork(&program, &program_args) {
        Err(e) => {
            eprintln!("Cannot fork child {}", e);
            eprintln!("{}", e.backtrace());
            return;
        }
        Ok(race) => race,
    };
    race.trace();

    if let Some(filename) = args.value_of("OUTFILE") {
        match fs::File::create(filename) {
            Ok(f) => {
                let mut bw = io::BufWriter::new(f);
                for l in tv::TreeView::new(race.tree()).gen_lines() {
                    if let Err(e) = writeln!(bw, "{}", l) {
                        eprintln!("Error dumping tree: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error opening file {}: {}", filename, e);
            }
        }
    }

    if args.is_present("TUI") {
        let tv = tv::TreeView::new(race.tree());
        let mut tui: tui::Tui<_, term::Term> = tui::Tui::new(tv).unwrap();
        tui.event_loop();
    }
}
