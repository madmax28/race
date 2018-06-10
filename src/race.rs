extern crate nix;

use self::nix::sys::{ptrace, signal, wait};
use self::nix::unistd;
pub use self::nix::unistd::Pid;

use std::collections::HashMap;
use std::ffi;
use std::fs;
use std::iter::Iterator;

use tui::{Tui, TuiSource};

macro_rules! debug {
    ($($arg:tt)+) => ({
        if ::std::env::var_os("RACE_LOG").is_some() {
            println!($($arg)+);
        }
    })
}

fn handle_nix_error(e: nix::Error) -> ! {
    panic!("{}", e);
}

pub fn fork_child(program: &str, args: &[String]) -> Pid {
    match unistd::fork() {
        Ok(unistd::ForkResult::Child) => {
            let mut cargs: Vec<ffi::CString> = args.iter()
                .cloned()
                .map(|a| ffi::CString::new(a).unwrap())
                .collect();
            child(&ffi::CString::new(program).unwrap(), &cargs);
        }
        Ok(unistd::ForkResult::Parent { child }) => child,
        Err(e) => handle_nix_error(e),
    }
}

fn child(program: &ffi::CString, args: &[ffi::CString]) -> ! {
    if let Err(e) = ptrace::traceme() {
        handle_nix_error(e);
    }

    if let Err(e) = unistd::execvp(&program, &args) {
        handle_nix_error(e);
    }

    unreachable!();
}

fn int_to_ptrace_event(i: i32) -> ptrace::Event {
    use self::ptrace::Event::*;

    match i {
        1 => PTRACE_EVENT_FORK,
        2 => PTRACE_EVENT_VFORK,
        3 => PTRACE_EVENT_CLONE,
        4 => PTRACE_EVENT_EXEC,
        5 => PTRACE_EVENT_VFORK_DONE,
        6 => PTRACE_EVENT_EXIT,
        7 => PTRACE_EVENT_SECCOMP,
        i => panic!("Invalid ptrace event: {}", i),
    }
}

#[derive(Debug)]
struct Process {
    pid: Pid,
    parent: Option<usize>,
    children: Vec<usize>,
    cmdline: String,
}

impl Process {
    fn new(pid: Pid) -> Self {
        Process {
            pid,
            parent: None,
            children: Vec::new(),
            cmdline: "UNKNOWN".to_string(),
        }
    }

    fn read_cmdline(&mut self) {
        let filename = format!("/proc/{}/cmdline", self.pid);
        self.cmdline = fs::read(&filename)
            .expect(&format!("Error reading {}", &filename))
            .iter_mut()
            .map(|c| if *c == 0 { ' ' } else { *c as char })
            .collect::<String>()
            .trim()
            .to_string();
    }
}

#[derive(Debug)]
struct ProcessTree {
    processes: Vec<Process>,
    pid_map: HashMap<Pid, usize>,
}

impl ProcessTree {
    fn new() -> Self {
        ProcessTree {
            processes: Vec::new(),
            pid_map: HashMap::new(),
        }
    }

    fn add(&mut self, p: Process) -> &mut Process {
        assert!(!self.pid_map.contains_key(&p.pid));

        let new_idx = self.processes.len();
        self.pid_map.insert(p.pid, new_idx);
        self.processes.push(p);
        &mut self.processes[new_idx]
    }

    fn get(&mut self, pid: Pid) -> Option<&mut Process> {
        let idx = self.pid_map.get(&pid)?;
        Some(&mut self.processes[*idx])
    }

    fn maps(&self, pid: Pid) -> bool {
        self.pid_map.contains_key(&pid)
    }

    fn add_child(&mut self, ppid: Pid, pid: Pid) {
        let child_idx = self.pid_map[&pid];
        let parent_idx = self.pid_map[&ppid];
        self.processes[child_idx].parent = Some(parent_idx);
        self.processes[parent_idx].children.push(child_idx);
    }
}

#[derive(Clone, Debug)]
struct ProcessDisplayOptions {
    expanded: bool,
}

impl ProcessDisplayOptions {
    fn new() -> Self {
        ProcessDisplayOptions { expanded: true }
    }
}

#[derive(Debug)]
struct ProcessTreeTui<'a> {
    pt: &'a ProcessTree,
    display_opts: Vec<ProcessDisplayOptions>,
    proc_lookup: Vec<usize>,
}

impl<'a> ProcessTreeTui<'a> {
    fn new(pt: &'a ProcessTree) -> Self {
        ProcessTreeTui {
            pt,
            display_opts: vec![ProcessDisplayOptions::new(); pt.processes.len()],
            proc_lookup: Vec::new(),
        }
    }
}

impl<'a> TuiSource for ProcessTreeTui<'a> {
    fn gen_lines(&mut self) -> Vec<String> {
        self.proc_lookup.clear();
        ProcessTreeIter::new(self.pt)
            .filter_map(|path| {
                // Hide collapsed sub-trees
                if path.iter()
                    .rev()
                    .skip(1)
                    .any(|idx| !self.display_opts[*idx].expanded)
                {
                    return None;
                }

                let idx = *path.last().unwrap();
                self.proc_lookup.push(idx);

                let mut prefix = String::new();
                if path.len() > 1 {
                    prefix.push_str(&path.iter()
                        .skip(1)
                        .take(path.len() - 2)
                        .map(|idx| {
                            // Check if this is the last sibling
                            if *idx == *self.pt.processes[self.pt.processes[*idx].parent.unwrap()]
                                .children
                                .last()
                                .unwrap()
                            {
                                "  "
                            } else {
                                "| "
                            }
                        })
                        .collect::<String>());
                    prefix.push_str("\\_ ");
                }

                let exp = if self.display_opts[idx].expanded {
                    "[+] "
                } else {
                    "[-] "
                };

                let content = &self.pt.processes[idx].cmdline;

                Some(format!("{}{}{}", prefix, exp, content))
            })
            .collect()
    }

    fn handle_char(&mut self, c: char, line: i32) {
        match c {
            ' ' => {
                let p = &mut self.display_opts[self.proc_lookup[line as usize]];
                if p.expanded {
                    p.expanded = false;
                } else {
                    p.expanded = true;
                }
            }
            _ => (),
        }
    }
}

#[derive(Debug)]
struct ProcessTreeIter<'a> {
    pt: &'a ProcessTree,
    frontier: Vec<Vec<usize>>,
}

impl<'a> ProcessTreeIter<'a> {
    fn new(pt: &'a ProcessTree) -> Self {
        ProcessTreeIter {
            pt,
            frontier: vec![vec![0]],
        }
    }
}

impl<'a> Iterator for ProcessTreeIter<'a> {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let path = self.frontier.pop()?;

        {
            let process = &self.pt.processes[*path.last().unwrap()];
            for child_idx in process.children.iter().rev() {
                let mut p = path.clone();
                p.push(*child_idx);
                self.frontier.push(p);
            }
        }

        Some(path)
    }
}

#[derive(Debug)]
pub struct Race {
    tracees: ProcessTree,
}

impl Race {
    pub fn new(pid: Pid) -> Self {
        let mut race = Race {
            tracees: ProcessTree::new(),
        };
        race.tracees.add(Process::new(pid));
        race
    }

    pub fn trace(&mut self) {
        while let Ok(result) = wait::waitpid(Pid::from_raw(-1), Some(wait::WaitPidFlag::__WALL)) {
            self.handle_wakeup(result);
        }
    }

    fn handle_wakeup(&mut self, res: wait::WaitStatus) {
        use self::signal::Signal::*;
        use self::wait::WaitStatus::*;

        debug!("Handling wakeup: {:?}", res);

        #[allow(unused_variables)]
        match res {
            Exited(pid, status) => (),
            Signaled(pid, sig, has_coredump) => (),
            Stopped(pid, sig) => {
                match sig {
                    SIGTRAP => {
                        // Only expected at initial stop of tracee
                        assert!(self.tracees.get(pid).is_some());
                        self.setopts(pid);
                        self.tracees.get(pid).unwrap().read_cmdline();
                        Race::cont(pid, None);
                    }
                    SIGSTOP => {
                        // Expected once per tracee on start
                        self.setopts(pid);

                        // TODO: Replace with if let block once NLL are supported
                        let p = if self.tracees.maps(pid) {
                            self.tracees.get(pid).unwrap()
                        } else {
                            self.tracees.add(Process::new(pid))
                        };
                        p.read_cmdline();

                        Race::cont(pid, None);
                    }
                    _ => {
                        debug!("Ignored");
                        Race::cont(pid, sig);
                    }
                }
            }
            PtraceEvent(pid, sig, ev) => {
                self.handle_ptrace_event(pid, sig, ev);
                Race::cont(pid, None);
            }
            PtraceSyscall(pid) => Race::cont(pid, None),
            Continued(pid) => unimplemented!(),
            StillAlive => unimplemented!(),
        }
    }

    fn handle_ptrace_event(&mut self, pid: Pid, sig: signal::Signal, ev: i32) {
        use self::ptrace::Event::*;

        let ev_msg = ptrace::getevent(pid).unwrap();
        let ev = int_to_ptrace_event(ev);

        debug!(
            "Handling ptrace event for {}, sig {:?}, event {:?} = {:?}",
            pid, sig, ev, ev_msg
        );

        assert!(self.tracees.get(pid).is_some());

        match ev {
            PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE => {
                let child_pid = Pid::from_raw(ev_msg as i32);
                if self.tracees.maps(child_pid) {
                    self.tracees.get(child_pid).unwrap()
                } else {
                    self.tracees.add(Process::new(child_pid))
                };
                self.tracees.add_child(pid, child_pid);
            }
            PTRACE_EVENT_EXEC => {
                self.tracees.get(pid).unwrap().read_cmdline();
            }
            PTRACE_EVENT_VFORK_DONE => (),
            PTRACE_EVENT_EXIT => (),
            PTRACE_EVENT_SECCOMP => (),
        }
    }

    fn setopts(&self, pid: Pid) {
        use self::ptrace::Options;

        let mut options = Options::PTRACE_O_TRACECLONE | Options::PTRACE_O_TRACEEXEC
            | Options::PTRACE_O_TRACEFORK | Options::PTRACE_O_TRACEVFORK
            | Options::PTRACE_O_TRACESYSGOOD | Options::PTRACE_O_EXITKILL;

        if let Err(_) = ptrace::setoptions(pid, options) {
            debug!("Warning: Setting options failed. Trying without PTRACE_O_EXITKILL");
            options.remove(Options::PTRACE_O_EXITKILL);
            if let Err(e) = ptrace::setoptions(pid, options) {
                handle_nix_error(e);
            }
        }
    }

    fn cont<T: Into<Option<signal::Signal>>>(pid: Pid, sig: T) {
        if let Err(e) = ptrace::cont(pid, sig) {
            handle_nix_error(e);
        }
    }

    pub fn dump_tree(&mut self) {
        Tui::run(&mut ProcessTreeTui::new(&self.tracees));
    }
}
