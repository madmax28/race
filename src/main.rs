mod args;
mod process;
mod race;
mod tui;
mod util;

use crate::tui::{term, tv};

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

fn main() {
    let args = args::parse_args();

    let program = args.value_of("PROGRAM").unwrap();
    let program_args: Vec<_> = args
        .values_of("ARGS")
        .and_then(|vs| Some(vs.collect()))
        .unwrap_or_default();

    let child = race::fork_child(&program, &program_args);
    let mut race = race::Race::new(child);
    race.trace();

    if args.is_present("TUI") {
        let tv = tv::TreeView::new(race.tree());
        let mut tui: tui::Tui<_, term::Term> = tui::Tui::new(tv).unwrap();
        tui.event_loop();
    }
}
