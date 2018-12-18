pub mod tree;

use nix::unistd::Pid;

use std::fs;
use std::io;

#[derive(Debug)]
pub struct ProcessData {
    pid: Pid,
    cmdline: String,
}

impl ProcessData {
    pub fn new(pid: Pid) -> Self {
        ProcessData {
            pid,
            cmdline: "UNKNOWN".to_string(),
        }
    }

    pub fn read_cmdline(&mut self) -> Result<(), io::Error> {
        let filename = format!("/proc/{}/cmdline", self.pid);
        self.cmdline = fs::read_to_string(&filename)?
            .replace(0 as char, " ")
            .replace('\n', " ");

        Ok(())
    }
}

#[derive(Debug)]
pub struct ProcessDataLineIter<'a> {
    proc_data: &'a ProcessData,
    done: bool,
}

impl<'a> ProcessDataLineIter<'a> {
    fn new(proc_data: &'a ProcessData) -> Self {
        ProcessDataLineIter {
            proc_data,
            done: false,
        }
    }
}

impl<'a> Iterator for ProcessDataLineIter<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            None
        } else {
            self.done = true;
            Some(self.proc_data.cmdline.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_data_line_iter() {
        let data = ProcessData {
            pid: Pid::from_raw(0),
            cmdline: "blablub".to_owned(),
        };
        let mut iter = ProcessDataLineIter::new(&data);
        assert_eq!(iter.next(), Some("blablub".to_string()));
        assert_eq!(iter.next(), None);

        let data = ProcessData {
            pid: Pid::from_raw(123),
            cmdline: "blab\nlub".to_owned(),
        };
        let mut iter = ProcessDataLineIter::new(&data);
        assert_eq!(iter.next(), Some("blab\nlub".to_string()));
        assert_eq!(iter.next(), None);
    }
}
