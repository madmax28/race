extern crate nix;

use self::nix::sys::{ptrace, signal, wait};
use self::nix::unistd;
pub use self::nix::unistd::Pid;

use std::collections::HashMap;
use std::ffi;
use std::fs;

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
        Ok(unistd::ForkResult::Parent { child }) => {
            child
        }
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

    panic!(); // Won't come here
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
    children: Vec<usize>,
    cmdline: String,
}

impl Process {
    fn new(pid: Pid) -> Self {
        Process {
            pid,
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
            .collect();
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
        let idx = self.pid_map[&pid];
        self.get(ppid).unwrap().children.push(idx)
    }

    fn dump_tree(&self) {
        self.dump_proc_recursive(0, 0);
    }

    fn dump_proc_recursive(&self, idx: usize, indent: i32) {
        let process = &self.processes[idx];

        for _ in 1..indent {
            print!("| ");
        }
        if indent > 0 {
            print!("\\_ ");
        }
        println!("{}", process.cmdline);
        for child in &process.children {
            self.dump_proc_recursive(*child, indent + 1);
        }
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
        while let Ok(result) = wait::wait() {
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
        if let Err(e) = ptrace::setoptions(pid, ptrace::Options::all()) {
            handle_nix_error(e);
        }
    }

    fn cont<T: Into<Option<signal::Signal>>>(pid: Pid, sig: T) {
        if let Err(e) = ptrace::cont(pid, sig) {
            handle_nix_error(e);
        }
    }

    pub fn dump_tree(&self) {
        self.tracees.dump_tree();
    }
}
