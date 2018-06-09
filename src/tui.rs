extern crate pancurses;

use self::pancurses::*;

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
    selected_line: i32,

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
            selected_line: 0,
            win_h: 0,
            win_w: 0,
            scroll: Pair::new(0, 0),
            scroll_max: Pair::new(0, 0),
        };
        noecho();
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
                    'j' => {
                        if tui.selected_line < tui.lines.len() as i32 - 1 {
                            tui.selected_line += 1;
                            tui.handle_scrolloff();
                        }
                    }
                    'k' => {
                        if tui.selected_line > 0 {
                            tui.selected_line -= 1;
                            tui.handle_scrolloff();
                        }
                    }
                    'l' => tui.scroll(Pair::new(SCROLL_SPEED, 0)),

                    '0' => tui.scroll.x = 0,
                    '$' => tui.scroll.x = ::std::cmp::max(0, tui.data_width() - tui.win_w),
                    'g' => {
                        tui.selected_line = 0;
                        tui.handle_scrolloff();
                    }
                    'G' => {
                        tui.selected_line = tui.lines.len() as i32 - 1;
                        tui.handle_scrolloff();
                    }
                    'd' => {
                        tui.selected_line = ::std::cmp::min(
                            tui.selected_line + tui.win_h / 2,
                            tui.lines.len() as i32 - 1,
                        );
                        tui.handle_scrolloff();
                    }
                    'u' => {
                        tui.selected_line = ::std::cmp::max(tui.selected_line - tui.win_h / 2, 0);
                        tui.handle_scrolloff();
                    }

                    _ => (),
                },
                Some(Input::KeyResize) => {
                    resize_term(0, 0);
                    tui.update_size();
                    tui.handle_scrolloff();
                }
                Some(_) => (),
                None => (),
            }
            tui.redraw();
        }

        endwin();
    }

    fn handle_scrolloff(&mut self) {
        let scrolloff = self.win_h / 4;

        let line_top = self.scroll.y;
        let line_bot = ::std::cmp::min((self.lines.len() as i32) - 1, line_top + self.win_h - 1);

        let diff_top = self.selected_line - line_top;
        let diff_bot = line_bot - self.selected_line;

        if diff_bot < scrolloff {
            self.scroll(Pair::new(0, scrolloff - diff_bot));
        } else if diff_top < scrolloff {
            self.scroll(Pair::new(0, diff_top - scrolloff));
        }
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
        }
    }

    fn redraw(&mut self) {
        self.win.clear();
        for (idx, line) in self.lines
            .iter()
            .enumerate()
            .skip(self.scroll.y as usize)
            .take(self.win_h as usize)
        {
            if self.selected_line == idx as i32 {
                self.win.attron(Attribute::Reverse);
            } else {
                self.win.attroff(Attribute::Reverse);
            }

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
        self.win.refresh();
    }

    fn update_size(&mut self) {
        self.win_h = self.win.get_max_y() - self.win.get_beg_y();
        self.win_w = self.win.get_max_x() - self.win.get_beg_x();

        let data_height = self.lines.len() as i32;
        let data_width = self.data_width();

        self.scroll_max.x = max(0, data_width - self.win_w);
        self.scroll_max.y = max(0, data_height - self.win_h);
    }

    fn data_width(&self) -> i32 {
        self.lines.iter().fold(0, |w, l| max(w, l.len())) as i32
    }
}
