mod args;
mod process;
mod race;
mod tui;
mod util;

use crate::process::tree::ProcessTree;
use crate::tui::{term, tv};

use std::fs;
use std::io;
use std::io::Write;
use std::path;

type Result<T> = std::result::Result<T, failure::Error>;

fn main() {
    let args = args::parse_args();

    // Trace
    let tree: ProcessTree = if let Some(filename) = args.value_of("INFILE") {
        match fs::File::open(filename) {
            Ok(f) => {
                let br = io::BufReader::new(f);
                match serde_json::from_reader(br) {
                    Ok(tree) => tree,
                    Err(e) => {
                        eprintln!("Error parsing json file {}: {}", filename, e);
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error open file {}: {}", filename, e);
                return;
            }
        }
    } else if let Some(program) = args.values_of("PROGRAM") {
        let program: Vec<_> = program.collect();

        let mut race = match race::Race::fork(&program) {
            Err(e) => {
                eprintln!("Cannot fork child {}", e);
                eprintln!("{}", e.backtrace());
                return;
            }
            Ok(race) => race,
        };
        race.trace();
        race.into_tree()
    } else {
        unreachable!()
    };

    // Dump db
    if args.is_present("PROGRAM") {
        let mut filename = "race.json".to_string();
        let mut n = 0;
        while path::Path::new(&filename).exists() {
            filename = format!("race.{}.json", n);
            n += 1;
        }

        match fs::File::create(&filename) {
            Ok(f) => {
                let mut bw = io::BufWriter::new(f);
                if let Err(e) = serde_json::to_writer_pretty(&mut bw, &tree) {
                    eprintln!("Error dumping db: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Error opening file {}: {}", filename, e);
            }
        }
    }

    if let Some(filename) = args.value_of("OUTFILE") {
        match fs::File::create(filename) {
            Ok(f) => {
                let mut bw = io::BufWriter::new(f);
                for l in tv::TreeView::new(&tree).gen_lines() {
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
        let tv = tv::TreeView::new(&tree);
        let mut tui: tui::Tui<_, term::Term> = tui::Tui::new(tv).unwrap();
        tui.event_loop();
    }
}
