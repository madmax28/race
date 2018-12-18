pub mod tv;

use pancurses::*;

use std::cmp::max;
use std::iter::Iterator;
use std::ops::Add;

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

trait TuiClient {
    fn gen_lines(&mut self) -> Vec<String>;
    fn handle_char(&mut self, c: char, line: i32);
}

#[derive(Debug)]
struct Tui<'a, T: 'a + TuiClient> {
    win: Window,
    src: &'a mut T,

    lines: Vec<String>,
    selected_line: i32,

    win_size: Pair,
    scroll: Pair,
    scroll_max: Pair,
}

impl<'a, T: TuiClient> Tui<'a, T> {
    fn new(src: &'a mut T) -> Self {
        let tui = Tui {
            win: initscr(),
            src,
            lines: Vec::new(),
            selected_line: 0,
            win_size: Pair::new(0, 0),
            scroll: Pair::new(0, 0),
            scroll_max: Pair::new(0, 0),
        };
        noecho();
        tui
    }

    fn run(src: &'a mut T) {
        let mut tui = Tui::new(src);

        tui.lines = tui.src.gen_lines();
        tui.update_size();
        tui.redraw();

        loop {
            match tui.win.getch() {
                Some(Input::Character(c)) => match c {
                    'q' => break,

                    'h' => {
                        let dist = -tui.win_size.x / 2;
                        tui.scroll(Pair::new(dist, 0))
                    }
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
                    'l' => {
                        let dist = tui.win_size.x / 2;
                        tui.scroll(Pair::new(dist, 0));
                    }

                    '0' => tui.scroll.x = 0,
                    '$' => tui.scroll.x = ::std::cmp::max(0, tui.data_width() - tui.win_size.x),
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
                            tui.selected_line + tui.win_size.y / 2,
                            tui.lines.len() as i32 - 1,
                        );
                        tui.handle_scrolloff();
                    }
                    'u' => {
                        tui.selected_line =
                            ::std::cmp::max(tui.selected_line - tui.win_size.y / 2, 0);
                        tui.handle_scrolloff();
                    }

                    c => {
                        tui.src.handle_char(c, tui.selected_line);
                        tui.lines = tui.src.gen_lines();
                        tui.update_size();
                    }
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
        let scrolloff = self.win_size.y / 4;

        let line_top = self.scroll.y;
        let line_bot = ::std::cmp::min(
            (self.lines.len() as i32) - 1,
            line_top + self.win_size.y - 1,
        );

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
        for (idx, line) in self
            .lines
            .iter()
            .enumerate()
            .skip(self.scroll.y as usize)
            .take(self.win_size.y as usize)
        {
            if self.selected_line == idx as i32 {
                self.win.attron(Attribute::Reverse);
            } else {
                self.win.attroff(Attribute::Reverse);
            }

            let idx = self.scroll.x as usize;
            let line_len = line.chars().count();
            if idx < line_len {
                self.win.addstr(
                    line.chars()
                        .skip(idx)
                        .take(self.win_size.x as usize)
                        .collect::<String>(),
                );
                if line_len - idx < self.win_size.x as usize {
                    self.win.addch('\n');
                }
            } else {
                self.win.addch('\n');
            }
        }
        self.win.refresh();
    }

    fn update_size(&mut self) {
        self.win_size.y = self.win.get_max_y() - self.win.get_beg_y();
        self.win_size.x = self.win.get_max_x() - self.win.get_beg_x();

        let data_height = self.lines.len() as i32;
        let data_width = self.data_width();

        self.scroll_max.x = max(0, data_width - self.win_size.x);
        self.scroll_max.y = max(0, data_height - self.win_size.y);
    }

    fn data_width(&self) -> i32 {
        self.lines.iter().fold(0, |w, l| max(w, l.chars().count())) as i32
    }
}
