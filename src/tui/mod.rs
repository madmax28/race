pub mod term;
pub mod tv;

use crate::util::{Point, Rect};
use crate::Result;

use std::sync::mpsc;

type AnsiColor = u8;

const DARK_GREY: AnsiColor = 234;
const LIGHT_GREY: AnsiColor = 236;
const WHITE: AnsiColor = 255;

pub trait Draw {
    fn draw(&mut self, rect: &Rect, frame: &mut Frame);
    fn dirty(&self) -> bool;
}

#[derive(Debug)]
pub enum Event {
    Input(termion::event::Event),
    TermResized,
}

#[derive(Debug, Clone, PartialEq)]
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

    fn clear(&mut self) {
        self.c = ' ';
        self.fg = WHITE;
        self.bg = DARK_GREY;
    }
}

#[derive(Debug)]
pub struct Frame {
    size: Point,
    cells: Vec<Cell>,
}

impl Frame {
    fn new(size: Point) -> Frame {
        let mut cells = Vec::with_capacity((size.x * size.y) as usize);
        for y in 0i32..size.y {
            for x in 0i32..size.x {
                cells.push(Cell::new(Point::new(x, y), ' '));
            }
        }

        Frame { size, cells }
    }

    fn clear_rect(&mut self, rect: &Rect) {
        for p in rect.points() {
            self.cell_mut(p).clear();
        }
    }

    fn cell_mut(&mut self, pos: Point) -> &mut Cell {
        &mut self.cells[(pos.x + pos.y * self.size.x) as usize]
    }

    fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter()
    }

    fn add(&mut self, cell: Cell) {
        let pos = cell.pos;
        self.cells[(pos.x + pos.y * self.size.x) as usize] = cell;
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
    fn handle_char(&mut self, c: char);
}

#[derive(Debug)]
pub struct Tui<C, B>
where
    C: Client + Draw,
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
    C: Client + Draw,
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
        tui.redraw(true);

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
                self.redraw(true);
            }
            Input(Key(Char(c))) => match c {
                'q' => return false,
                c => {
                    self.client.handle_char(*c);
                    self.redraw(false);
                }
            },
            _ => (),
        }

        true
    }

    fn redraw(&mut self, force: bool) {
        let frame = self.backend.get_frame_mut();

        let draw = if self.client.dirty() || force {
            let rect = Rect::new(Point::new(0, 0), self.size - Point::new(1, 1));
            self.client.draw(&rect, frame);
            true
        } else {
            false
        };

        if draw {
            self.backend.draw(false);
        }
    }

    fn update_size(&mut self) {
        self.size = self.backend.update_size();
    }
}
