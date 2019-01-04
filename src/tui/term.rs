use termion::input::TermReadEventsAndRaw;
use termion::raw::IntoRawMode;
use termion::{color, cursor, raw};

use nix::sys::signal;

use crate::tui;
use crate::Result;

use std::io;
use std::io::Write;
use std::sync::atomic;
use std::sync::mpsc;
use std::thread;
use std::time;

static mut RECVD_SIGWINCH: atomic::AtomicBool = atomic::AtomicBool::new(false);

extern "C" fn handle_sigwinch(_: libc::c_int) {
    unsafe {
        RECVD_SIGWINCH.store(true, atomic::Ordering::Relaxed);
    }
}

pub struct Term {
    size: (u16, u16),
    stdout: raw::RawTerminal<io::Stdout>,

    frame_idx: usize,
    frame_buf: [tui::Frame; 2],
}

impl Drop for Term {
    fn drop(&mut self) {
        write!(self.stdout, "{}", cursor::Show).unwrap();
    }
}

impl tui::Backend for Term {
    fn new(channel: mpsc::SyncSender<tui::Event>) -> Result<Self> {
        let mut term = Term {
            size: (0, 0),
            stdout: io::stdout().into_raw_mode()?,

            frame_idx: 0,
            frame_buf: [
                tui::Frame::empty(tui::Point::new(0, 0)),
                tui::Frame::empty(tui::Point::new(0, 0)),
            ],
        };
        write!(term.stdout, "{}", cursor::Hide).unwrap();

        // Install sig handler
        {
            let sighandler = signal::SigAction::new(
                signal::SigHandler::Handler(handle_sigwinch),
                signal::SaFlags::SA_RESTART,
                signal::SigSet::all(),
            );
            unsafe {
                signal::sigaction(signal::Signal::SIGWINCH, &sighandler).unwrap();
            }
        }

        // Spawn termion event listener
        let (termion_tx, termion_rx) = mpsc::sync_channel(100);
        {
            thread::spawn(move || {
                for ev in io::stdin().events_and_raw() {
                    match ev {
                        Ok((ev, _)) => {
                            if termion_tx.send(tui::Event::Input(ev)).is_err() {
                                return;
                            }
                        }
                        Err(_) => {
                            return;
                        }
                    }
                }
            });
        }

        // Spawn listener
        thread::spawn(move || loop {
            match termion_rx.try_recv() {
                Ok(ev) => {
                    if channel.send(ev).is_err() {
                        return;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => (),
                Err(_) => return,
            }

            if unsafe { RECVD_SIGWINCH.compare_and_swap(true, false, atomic::Ordering::Relaxed) }
                && channel.send(tui::Event::TermResized).is_err()
            {
                return;
            }

            thread::sleep(time::Duration::from_millis(10));
        });

        Ok(term)
    }

    fn draw(&mut self, force: bool) {
        let frame = &self.frame_buf[self.frame_idx];
        let old_frame = &self.frame_buf[(self.frame_idx + 1) % 2];

        for (idx, cell) in frame.cells().enumerate() {
            if *cell != old_frame.cells[idx] || force {
                write!(
                    self.stdout,
                    "{}",
                    cursor::Goto(cell.pos.x as u16 + 1, cell.pos.y as u16 + 1)
                    )
                    .unwrap();
                write!(self.stdout, "{}", color::Fg(color::AnsiValue(cell.fg))).unwrap();
                write!(self.stdout, "{}", color::Bg(color::AnsiValue(cell.bg))).unwrap();
                write!(self.stdout, "{}", cell.c).unwrap();
            }
        }
        self.stdout.flush().unwrap();

        self.frame_idx = (self.frame_idx + 1) % 2;
    }

    fn update_size(&mut self) -> tui::Point {
        self.size = match termion::terminal_size() {
            Ok(sz) => sz,
            Err(_) => {
                panic!();
            }
        };

        let size = tui::Point::new(i32::from(self.size.0), i32::from(self.size.1));
        self.frame_idx = 0;
        self.frame_buf = [
            tui::Frame::empty(size),
            tui::Frame::empty(size),
        ];
        self.draw(true);

        size
    }

    fn get_frame_mut(&mut self) -> &mut tui::Frame {
        &mut self.frame_buf[self.frame_idx]
    }
}
