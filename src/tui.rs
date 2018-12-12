use pancurses::*;

use crate::tree::{Tree, TreeIter};

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
        for (idx, line) in self.lines
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

pub trait TuiTree {
    type Iter: Iterator<Item = Vec<usize>>;
    type NodeIter: Iterator<Item = String>;

    fn size(&self) -> usize;
    fn iter(&self) -> Self::Iter;
    fn next_sibling(&self, node: usize) -> Option<usize>;
    fn node_iter(&self, node: usize) -> Self::NodeIter;
}

impl<'a, T> TuiTree for &'a Tree<T>
where
    &'a T: IntoIterator<Item = String>,
{
    type Iter = TreeIter<'a, T>;
    type NodeIter = <&'a T as IntoIterator>::IntoIter;

    fn size(&self) -> usize {
        Tree::num_nodes(self)
    }

    fn iter(&self) -> Self::Iter {
        Tree::iter(self)
    }

    fn next_sibling(&self, node: usize) -> Option<usize> {
        Tree::next_sibling(self, node)
    }

    fn node_iter(&self, node: usize) -> Self::NodeIter {
        Tree::get(self, node).data().into_iter()
    }
}

#[derive(Debug)]
pub struct TreeTui<T: TuiTree> {
    tree: T,
    expanded: Vec<bool>,
    lookup: Vec<usize>,
}

impl<T> TreeTui<T>
where
    T: TuiTree,
{
    fn new(tree: T) -> Self {
        let size = tree.size();
        TreeTui {
            tree,
            expanded: vec![true; size],
            lookup: Vec::new(),
        }
    }

    pub fn run(tree: T) {
        let mut tt = TreeTui::new(tree);
        Tui::run(&mut tt);
    }
}

impl<T> TuiClient for TreeTui<T>
where
    T: TuiTree,
{
    fn gen_lines(&mut self) -> Vec<String> {
        TreeTuiIter::new(self).collect()
    }

    fn handle_char(&mut self, c: char, line: i32) {
        match c {
            ' ' => {
                let id: usize = self.lookup[line as usize];
                if self.expanded[id] {
                    self.expanded[id] = false;
                } else {
                    self.expanded[id] = true;
                }
            }
            _ => (),
        }
    }
}

fn gen_path_prefix<T: TuiTree>(tree: &T, path: &[usize]) -> String {
    match path.len() {
        0 => panic!("Empty node path"),
        1...2 => "".to_string(),
        _ => path[1..path.len() - 1]
            .iter()
            .map(|id| {
                if tree.next_sibling(*id).is_some() {
                    "    │   "
                } else {
                    "        "
                }
            })
            .collect::<String>(),
    }
}

fn gen_line_prefix<T: TuiTree>(tt: &TreeTui<T>, path: &[usize], is_first_line: bool) -> String {
    let id = *path.last().unwrap();

    let expand_marker = if tt.expanded[id] { "[+] " } else { "[-] " };
    match (
        path.len(),
        is_first_line,
        tt.tree.next_sibling(id).is_some(),
    ) {
        (0...1, true, _) => expand_marker.to_string(),
        (0...1, false, _) => "    ".to_string(),
        (_, true, true) => format!("    ├── {}", expand_marker),
        (_, true, false) => format!("    └── {}", expand_marker),
        (_, false, true) => "    │       ".to_string(),
        (_, false, false) => "            ".to_string(),
    }
}

#[derive(Debug)]
enum TreeTuiIterState {
    Node,
    Line,
}

struct TreeTuiIter<'a, T: 'a + TuiTree> {
    state: TreeTuiIterState,
    tt: &'a mut TreeTui<T>,

    node_iter: T::Iter,
    path: Vec<usize>,
    node_prefix: String,

    line_iter: Option<T::NodeIter>,
    is_first_line: bool,
    line_prefix: String,
}

impl<'a, T> TreeTuiIter<'a, T>
where
    T: TuiTree,
{
    fn new(tt: &'a mut TreeTui<T>) -> Self {
        tt.lookup.clear();
        let node_iter = tt.tree.iter();
        TreeTuiIter {
            state: TreeTuiIterState::Node,
            tt,

            node_iter,
            path: Vec::new(),
            node_prefix: String::new(),

            line_iter: None,
            is_first_line: true,
            line_prefix: String::new(),
        }
    }
}

impl<'a, T> Iterator for TreeTuiIter<'a, T>
where
    T: TuiTree,
{
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            TreeTuiIterState::Node => {
                loop {
                    self.path = self.node_iter.next()?;
                    if self.path
                        .iter()
                        .rev()
                        .skip(1)
                        .all(|id| self.tt.expanded[*id])
                    {
                        break;
                    }
                }
                self.state = TreeTuiIterState::Line;
                self.node_prefix = gen_path_prefix(&self.tt.tree, &self.path);
                self.line_iter = Some(self.tt.tree.node_iter(*self.path.last().unwrap()));
                self.is_first_line = true;
                self.next()
            }
            TreeTuiIterState::Line => {
                if self.is_first_line {
                    self.line_prefix = gen_line_prefix(&self.tt, &self.path, true).to_string();
                }

                let res = {
                    if let Some(string) = self.line_iter.as_mut().unwrap().next() {
                        self.tt.lookup.push(*self.path.last().unwrap());
                        Some(format!(
                            "{}{}{}",
                            self.node_prefix, self.line_prefix, string
                        ))
                    } else {
                        self.state = TreeTuiIterState::Node;
                        return self.next();
                    }
                };

                if self.is_first_line {
                    self.is_first_line = false;
                    self.line_prefix = gen_line_prefix(&self.tt, &self.path, false).to_string();
                }

                res
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree::{Node, NodeId, Tree};
    use tui::TreeTuiIter;

    use std::collections::HashMap;
    use std::iter::{IntoIterator, Iterator};

    struct NodeMock {
        num_lines: u32,
        name: String,
    }

    impl NodeMock {
        fn new<T: Into<String>>(num_lines: u32, name: T) -> Self {
            NodeMock {
                num_lines,
                name: name.into(),
            }
        }
    }

    impl<'a> IntoIterator for &'a NodeMock {
        type Item = String;
        type IntoIter = NodeIterMock;

        fn into_iter(self) -> Self::IntoIter {
            NodeIterMock {
                num_lines: self.num_lines,
                name: self.name.clone(),
            }
        }
    }

    struct NodeIterMock {
        num_lines: u32,
        name: String,
    }

    impl Iterator for NodeIterMock {
        type Item = String;
        fn next(&mut self) -> Option<String> {
            if self.num_lines > 0 {
                self.num_lines -= 1;
                Some(format!("{}_line_{}", self.name, self.num_lines))
            } else {
                None
            }
        }
    }

    fn make_tree(n: u32) -> (Tree<NodeMock>, HashMap<String, NodeId>) {
        let mut t = Tree::new(Node::new(NodeMock {
            num_lines: n,
            name: "root".to_string(),
        }));

        let mut ids = HashMap::new();
        ids.insert("root".to_string(), 0);

        for (name, parent) in &vec![
            ("n1", "root"),
            ("n2", "root"),
            ("n3", "root"),
            ("n11", "n1"),
            ("n12", "n1"),
            ("n31", "n3"),
            ("n32", "n3"),
            ("n111", "n11"),
            ("n311", "n31"),
            ("n1111", "n111"),
            ("n3111", "n311"),
        ] {
            let parent = ids[&parent.to_string()];
            let id = t.insert(Node::new(NodeMock::new(n, name.to_string())), Some(parent));
            ids.insert(name.to_string(), id);
        }

        (t, ids)
    }

    #[test]
    fn iter() {
        let (t, ids) = make_tree(1);
        let mut tt = TreeTui::new(&t);

        let expected_lines = vec![
            "[+] root_line_0",
            "    ├── [+] n1_line_0",
            "    │       ├── [+] n11_line_0",
            "    │       │       └── [+] n111_line_0",
            "    │       │               └── [+] n1111_line_0",
            "    │       └── [+] n12_line_0",
            "    ├── [+] n2_line_0",
            "    └── [+] n3_line_0",
            "            ├── [+] n31_line_0",
            "            │       └── [+] n311_line_0",
            "            │               └── [+] n3111_line_0",
            "            └── [+] n32_line_0",
        ];
        let expected_ids = vec![
            ids["root"],
            ids["n1"],
            ids["n11"],
            ids["n111"],
            ids["n1111"],
            ids["n12"],
            ids["n2"],
            ids["n3"],
            ids["n31"],
            ids["n311"],
            ids["n3111"],
            ids["n32"],
        ];

        let mut line_count = 0;
        for (idx, line) in TreeTuiIter::new(&mut tt).enumerate() {
            line_count += 1;
            assert_eq!(line, expected_lines[idx]);
        }
        assert_eq!(line_count, expected_lines.len());
        for idx in 0..line_count {
            assert_eq!(tt.lookup[idx], expected_ids[idx]);
        }
    }

    #[test]
    fn iter_2lines() {
        let (t, ids) = make_tree(2);
        let mut tt = TreeTui::new(&t);

        let expected_lines = vec![
            "[+] root_line_1",
            "    root_line_0",
            "    ├── [+] n1_line_1",
            "    │       n1_line_0",
            "    │       ├── [+] n11_line_1",
            "    │       │       n11_line_0",
            "    │       │       └── [+] n111_line_1",
            "    │       │               n111_line_0",
            "    │       │               └── [+] n1111_line_1",
            "    │       │                       n1111_line_0",
            "    │       └── [+] n12_line_1",
            "    │               n12_line_0",
            "    ├── [+] n2_line_1",
            "    │       n2_line_0",
            "    └── [+] n3_line_1",
            "            n3_line_0",
            "            ├── [+] n31_line_1",
            "            │       n31_line_0",
            "            │       └── [+] n311_line_1",
            "            │               n311_line_0",
            "            │               └── [+] n3111_line_1",
            "            │                       n3111_line_0",
            "            └── [+] n32_line_1",
            "                    n32_line_0",
        ];
        let expected_ids = vec![
            ids["root"],
            ids["root"],
            ids["n1"],
            ids["n1"],
            ids["n11"],
            ids["n11"],
            ids["n111"],
            ids["n111"],
            ids["n1111"],
            ids["n1111"],
            ids["n12"],
            ids["n12"],
            ids["n2"],
            ids["n2"],
            ids["n3"],
            ids["n3"],
            ids["n31"],
            ids["n31"],
            ids["n311"],
            ids["n311"],
            ids["n3111"],
            ids["n3111"],
            ids["n32"],
            ids["n32"],
        ];

        let mut line_count = 0;
        for (idx, line) in TreeTuiIter::new(&mut tt).enumerate() {
            line_count += 1;
            assert_eq!(line, expected_lines[idx]);
        }
        assert_eq!(line_count, expected_lines.len());
        for idx in 0..line_count {
            assert_eq!(tt.lookup[idx], expected_ids[idx]);
        }
    }
}
