pub mod term;
pub mod tv;

use crate::util::Point;
use crate::Result;

use std::cmp;
use std::sync::mpsc;

type AnsiColor = u8;

const DARK_GREY: AnsiColor = 234;
const LIGHT_GREY: AnsiColor = 236;
const WHITE: AnsiColor = 255;

#[derive(Debug)]
pub enum Event {
    Input(termion::event::Event),
    TermResized,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Cell {
    pos: Point,
    c: char,
    fg: AnsiColor,
    bg: AnsiColor,
}

impl Cell {
    fn new(pos: Point, c: char) -> Self {
        Cell {
            pos,
            c,
            fg: WHITE,
            bg: DARK_GREY,
        }
    }
}

#[derive(Debug)]
pub struct Frame {
    size: Point,
    cells: Vec<Cell>,
}

impl Frame {
    fn empty(size: Point) -> Frame {
        let mut cells = Vec::with_capacity((size.x * size.y) as usize);
        for y in 0i32..size.y {
            for x in 0i32..size.x {
                cells.push(Cell::new(Point::new(x, y), ' '));
            }
        }

        Frame { size, cells }
    }

    fn clear(&mut self) {
        let empty = Frame::empty(self.size);
        self.cells = empty.cells;
    }

    fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter()
    }

    fn cells_mut(&mut self) -> impl Iterator<Item = &mut Cell> {
        self.cells.iter_mut()
    }

    fn add(&mut self, cell: Cell) {
        self.cells[cell.pos.x as usize + cell.pos.y as usize * self.size.x as usize] = cell;
    }
}

pub trait Backend
where
    Self: Sized,
{
    fn new(channel: mpsc::SyncSender<Event>) -> Result<Self>;
    fn draw(&mut self, redraw: bool);
    fn update_size(&mut self) -> Point;
    fn get_frame_mut(&mut self) -> &mut Frame;
}

pub trait Client {
    fn gen_lines(&mut self) -> Vec<String>;
    fn handle_char(&mut self, c: char, line: i32);
}

#[derive(Debug)]
pub struct Tui<C, B>
where
    C: Client,
    B: Backend,
{
    client: C,
    backend: B,
    evq: mpsc::Receiver<Event>,

    lines: Vec<String>,
    selected_line: i32,

    size: Point,
    scroll: Point,
    scroll_max: Point,
}

impl<C, B> Tui<C, B>
where
    C: Client,
    B: Backend,
{
    pub fn new(mut client: C) -> Result<Self> {
        let (tx, evq) = mpsc::sync_channel(100);
        let lines = client.gen_lines();
        let mut tui = Tui {
            client,
            backend: B::new(tx)?,
            evq,

            lines,
            selected_line: 0,

            size: Point::new(0, 0),
            scroll: Point::new(0, 0),
            scroll_max: Point::new(0, 0),
        };
        tui.update_size();
        tui.redraw();

        Ok(tui)
    }

    pub fn event_loop(&mut self) {
        loop {
            match self.evq.recv() {
                Ok(ev) => {
                    if !self.handle_event(&ev) {
                        return;
                    }
                }
                Err(_) => {
                    return;
                }
            }
        }
    }

    fn handle_event(&mut self, ev: &Event) -> bool {
        use self::Event::*;
        use termion::event::Event::*;
        use termion::event::Key::*;

        match ev {
            TermResized => {
                self.update_size();
                self.redraw();
            },
            Input(Key(Char(c))) => match c {
                'q' => return false,
                'h' => {
                    let dist = -self.size.x / 2;
                    self.scroll(Point::new(dist, 0));
                    self.redraw();
                }
                'j' => {
                    if self.selected_line < self.lines.len() as i32 - 1 {
                        self.selected_line += 1;
                        self.handle_scrolloff();
                        self.redraw();
                    }
                }
                'k' => {
                    if self.selected_line > 0 {
                        self.selected_line -= 1;
                        self.handle_scrolloff();
                        self.redraw();
                    }
                }
                'l' => {
                    let dist = self.size.x / 2;
                    self.scroll(Point::new(dist, 0));
                    self.redraw();
                }

                '0' => {
                    self.scroll.x = 0;
                    self.redraw();
                }
                '$' => {
                    self.scroll.x = cmp::max(0, self.data_width() - self.size.x);
                    self.redraw();
                }
                'g' => {
                    self.selected_line = 0;
                    self.handle_scrolloff();
                    self.redraw();
                }
                'G' => {
                    self.selected_line = self.lines.len() as i32 - 1;
                    self.handle_scrolloff();
                    self.redraw();
                }
                'd' => {
                    self.selected_line = cmp::min(
                        self.selected_line + self.size.y / 2,
                        self.lines.len() as i32 - 1,
                    );
                    self.handle_scrolloff();
                    self.redraw();
                }
                'u' => {
                    self.selected_line = cmp::max(self.selected_line - self.size.y / 2, 0);
                    self.handle_scrolloff();
                    self.redraw();
                }

                c => {
                    self.client.handle_char(*c, self.selected_line);
                    self.lines = self.client.gen_lines();
                    self.redraw();
                }
            },
            _ => (),
        }

        true
    }

    fn redraw(&mut self) {
        let frame = self.backend.get_frame_mut();
        frame.clear();

        for (y, l) in self
            .lines
            .iter()
            .skip(self.scroll.y as usize)
            .take(self.size.y as usize)
            .enumerate()
        {
            for (x, c) in l
                .chars()
                .skip(self.scroll.x as usize)
                .take(self.size.x as usize)
                .enumerate()
            {
                frame.add(Cell::new(Point::new(x as i32, y as i32), c));
            }
        }

        for cell in frame.cells_mut() {
            if cell.pos.y == self.selected_line - self.scroll.y {
                cell.bg = LIGHT_GREY;
            }
        }

        self.backend.draw(false);
    }

    fn scroll(&mut self, diff: Point) {
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

    fn handle_scrolloff(&mut self) {
        let scrolloff = self.size.y / 4;

        let line_top = self.scroll.y;
        let line_bot = cmp::min((self.lines.len() as i32) - 1, line_top + self.size.y - 1);

        let diff_top = self.selected_line - line_top;
        let diff_bot = line_bot - self.selected_line;

        if diff_bot < scrolloff {
            self.scroll(Point::new(0, scrolloff - diff_bot));
        } else if diff_top < scrolloff {
            self.scroll(Point::new(0, diff_top - scrolloff));
        }
    }

    fn data_width(&self) -> i32 {
        self.lines
            .iter()
            .fold(0, |w, l| cmp::max(w, l.chars().count())) as i32
    }

    fn update_size(&mut self) {
        self.size = self.backend.update_size();

        self.scroll_max.x = cmp::max(0, self.data_width() as i32 - self.size.x);
        self.scroll_max.y = cmp::max(0, self.lines.len() as i32 - self.size.y);
    }
}
