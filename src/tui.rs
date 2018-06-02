extern crate pancurses;

use self::pancurses::{Input, Window};

use std::cmp::max;
use std::ops::Add;

const SCROLL_SPEED: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pair {
    x: i32,
    y: i32,
}

impl Pair {
    fn new(x: i32, y: i32) -> Self {
        Pair { x, y }
    }
}

impl Add for Pair {
    type Output = Pair;

    fn add(self, rhs: Pair) -> Self::Output {
        Pair::new(self.x + rhs.x, self.y + rhs.y)
    }
}

pub trait AsLines {
    fn as_lines(&self) -> Vec<String>;
}

#[derive(Debug)]
pub struct Tui<'a, T: 'a + AsLines> {
    win: Window,
    src: &'a T,
    lines: Vec<String>,
    win_h: i32,
    win_w: i32,
    scroll: Pair,
    scroll_max: Pair,
}

impl<'a, T: AsLines> Tui<'a, T> {
    fn new(src: &'a T) -> Self {
        let tui = Tui {
            win: pancurses::initscr(),
            src,
            lines: Vec::new(),
            win_h: 0,
            win_w: 0,
            scroll: Pair::new(0, 0),
            scroll_max: Pair::new(0, 0),
        };
        pancurses::noecho();
        tui
    }

    pub fn run(src: &'a T) {
        let mut tui = Tui::new(src);

        tui.lines = tui.src.as_lines();
        tui.update_size();
        tui.redraw();

        loop {
            match tui.win.getch() {
                Some(Input::Character(c)) => match c {
                    'q' => break,
                    'h' => tui.scroll(Pair::new(-SCROLL_SPEED, 0)),
                    'j' => tui.scroll(Pair::new(0, SCROLL_SPEED)),
                    'k' => tui.scroll(Pair::new(0, -SCROLL_SPEED)),
                    'l' => tui.scroll(Pair::new(SCROLL_SPEED, 0)),
                    _ => (),
                },
                Some(Input::KeyResize) => {
                    pancurses::resize_term(0, 0);
                    tui.update_size();
                    tui.redraw();
                }
                Some(_) => (),
                None => (),
            }
        }
        pancurses::endwin();
    }

    fn scroll(&mut self, diff: Pair) {
        let mut new_scroll = self.scroll + diff;

        if new_scroll.x < 0 {
            new_scroll.x = 0;
        } else if new_scroll.x > self.scroll_max.x {
            new_scroll.x = self.scroll_max.x;
        }
        if new_scroll.y < 0 {
            new_scroll.y = 0;
        } else if new_scroll.y > self.scroll_max.y {
            new_scroll.y = self.scroll_max.y;
        }

        if new_scroll != self.scroll {
            self.scroll = new_scroll;
            self.redraw();
        }
    }

    fn redraw(&mut self) {
        self.win.clear();
        for line in self.lines
            .iter()
            .skip(self.scroll.y as usize)
            .take(self.win_h as usize)
        {
            let idx = self.scroll.x as usize;
            if idx < line.len() {
                self.win.addnstr(&line[idx..], (self.win_w) as usize);
                if line.len() - idx < self.win_w as usize {
                    self.win.addch('\n');
                }
            } else {
                self.win.addch('\n');
            }
        }
    }

    fn update_size(&mut self) {
        self.win_h = self.win.get_max_y() - self.win.get_beg_y();
        self.win_w = self.win.get_max_x() - self.win.get_beg_x();

        let (data_height, data_width) = (
            self.lines.len() as i32,
            self.lines.iter().fold(0, |w, l| max(w, l.len())) as i32,
        );

        self.scroll_max.x = max(0, data_width - self.win_w);
        self.scroll_max.y = max(0, data_height - self.win_h);
    }
}
