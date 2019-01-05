use nix::sys::{ptrace, signal, wait};
use nix::unistd;
use nix::unistd::Pid;

use failure::ResultExt;

use crate::process::tree::{NodeId, ProcessTree};
use crate::process::ProcessData;
use crate::Result;

use std::collections::HashMap;
use std::ffi;
use std::process;

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

fn child(program: &ffi::CString, args: &[ffi::CString]) -> ! {
    if let Err(e) = ptrace::traceme() {
        eprintln!("traceme(): {}", e);
        process::exit(-1);
    }

    if let Err(e) = unistd::execvp(&program, &args) {
        eprintln!("execvp(): {}", e);
        process::exit(-1);
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
pub struct Race {
    pt: ProcessTree,
    pid_map: HashMap<Pid, NodeId>,
}

impl Race {
    fn new(pid: Pid) -> Self {
        let root = ProcessData::new(pid);
        let mut race = Race {
            pt: ProcessTree::new(root),
            pid_map: HashMap::new(),
        };
        race.pid_map.insert(pid, 0);
        race
    }

    pub fn fork(program: &str, args: &[&str]) -> Result<Self> {
        let cargs: Vec<ffi::CString> = [program]
            .iter()
            .chain(args)
            .cloned()
            .map(|a| {
                Ok(ffi::CString::new(a)
                    .with_context(|e| format!("Invalid argument {}: {}", a, e))?)
            })
            .collect::<Result<_>>()?;

        match unistd::fork()? {
            unistd::ForkResult::Child => child(&cargs[0], &cargs),
            unistd::ForkResult::Parent { child } => Ok(Race::new(child)),
        }
    }

    pub fn trace(&mut self) {
        while let Ok(result) = wait::waitpid(Pid::from_raw(-1), Some(wait::WaitPidFlag::__WALL)) {
            self.handle_wakeup(result);
        }
    }

    pub fn tree(&self) -> &ProcessTree {
        &self.pt
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
                        assert!(self.pid_map.contains_key(&pid));

                        self.setopts(pid);
                        self.read_cmdline(pid);
                        Race::cont(pid, None);
                    }
                    SIGSTOP => {
                        // Expected once per tracee on start
                        self.setopts(pid);
                        if !self.pid_map.contains_key(&pid) {
                            let id = self.pt.insert(ProcessData::new(pid), None);
                            self.pid_map.insert(pid, id);
                        }
                        self.read_cmdline(pid);
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

        assert!(self.pid_map.contains_key(&pid));

        match ev {
            PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK | PTRACE_EVENT_CLONE => {
                let child_pid = Pid::from_raw(ev_msg as i32);
                if !self.pid_map.contains_key(&child_pid) {
                    let id = self
                        .pt
                        .insert(ProcessData::new(child_pid), Some(self.pid_map[&pid]));
                    self.pid_map.insert(child_pid, id);
                } else {
                    self.pt
                        .set_parent(self.pid_map[&child_pid], self.pid_map[&pid]);
                }
            }
            PTRACE_EVENT_EXEC => {
                self.read_cmdline(pid);
            }
            PTRACE_EVENT_VFORK_DONE => (),
            PTRACE_EVENT_EXIT => (),
            PTRACE_EVENT_SECCOMP => (),
        }
    }

    fn setopts(&self, pid: Pid) {
        use self::ptrace::Options;

        let mut options = Options::PTRACE_O_TRACECLONE
            | Options::PTRACE_O_TRACEEXEC
            | Options::PTRACE_O_TRACEFORK
            | Options::PTRACE_O_TRACEVFORK
            | Options::PTRACE_O_TRACESYSGOOD
            | Options::PTRACE_O_EXITKILL;

        if let Err(_e) = ptrace::setoptions(pid, options) {
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

    fn read_cmdline(&mut self, pid: Pid) {
        assert!(self.pid_map.contains_key(&pid));

        self.pt
            .get_mut(self.pid_map[&pid])
            .data_mut()
            .read_cmdline()
            .unwrap();
    }
}
