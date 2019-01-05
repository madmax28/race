use clap::clap_app;

pub type Args<'a> = clap::ArgMatches<'a>;

pub fn parse_args<'a>() -> Args<'a> {
    clap_app!(race =>
        (@setting AllowLeadingHyphen)
        (version: "0.1.0")
        (about: "Process tracer")
        (@arg TUI: -t --tui "Interactive TUI")
        (@arg OUTFILE: -o +takes_value "Dumps tree to file")
        (@arg PROGRAM: +required "Program to trace")
        (@arg ARGS: ... "Program args")
    ).get_matches()
}
