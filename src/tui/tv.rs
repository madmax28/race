use crate::tui;
use crate::util::Point;

use std::cmp;

pub trait Tree {
    type NodeIter: Iterator<Item = Vec<usize>>;
    type LineIter: Iterator<Item = String>;

    fn size(&self) -> usize;
    fn next_sibling(&self, node: usize) -> Option<usize>;

    fn node_iter(&self) -> Self::NodeIter;
    fn line_iter(&self, node: usize) -> Self::LineIter;
}

#[derive(Debug)]
pub struct TreeView<T: Tree> {
    tree: T,
    lines: Vec<String>,

    expanded: Vec<bool>,
    lookup: Vec<usize>,

    size: Point,
    data_size: Point,
    scroll: Point,
    scroll_max: Point,
    selected_line: i32,

    dirty: bool,
}

impl<T: Tree> TreeView<T> {
    pub fn new(tree: T) -> Self {
        let size = tree.size();
        let mut tv = TreeView {
            tree,
            lines: Vec::new(),

            expanded: vec![true; size],
            lookup: Vec::new(),

            size: Point::new(0, 0),
            data_size: Point::new(0, 0),
            scroll: Point::new(0, 0),
            scroll_max: Point::new(0, 0),
            selected_line: 0,

            dirty: false,
        };
        tv.fetch_lines();
        tv
    }

    fn fetch_lines(&mut self) {
        self.lines = TVLineIter::new(self).collect();

        self.data_size.y = self.lines.len() as i32;
        self.data_size.x = self
            .lines
            .iter()
            .map(|s| s.chars().count())
            .max()
            .unwrap_or(0) as i32;

        self.update_scroll_max();
    }

    fn update_size(&mut self, size: Point) {
        if self.size != size {
            self.size = size;
            self.update_scroll_max();
            self.scroll(0, 0);
        }
    }

    fn scroll(&mut self, dx: i32, dy: i32) {
        let old_scroll = self.scroll;

        self.scroll += Point::new(dx, dy);
        self.scroll.x = cmp::min(self.scroll.x, self.scroll_max.x);
        self.scroll.y = cmp::min(self.scroll.y, self.scroll_max.y);
        self.scroll.x = cmp::max(self.scroll.x, 0);
        self.scroll.y = cmp::max(self.scroll.y, 0);

        if self.scroll != old_scroll {
            self.dirty = true;
        }
    }

    fn scroll_beg(&mut self) {
        if self.scroll.x > 0 {
            self.scroll.x = 0;
            self.dirty = true;
        }
    }

    fn scroll_end(&mut self) {
        if self.scroll.x < self.scroll_max.x {
            self.scroll.x = self.scroll_max.x;
            self.dirty = true;
        }
    }

    fn update_scroll_max(&mut self) {
        self.scroll_max.x = cmp::max(0, self.data_size.x - self.size.x);
        self.scroll_max.y = cmp::max(0, self.data_size.y - self.size.y);
    }

    fn handle_scrolloff(&mut self) {
        let scrolloff = self.size.y / 4;

        let line_top = self.scroll.y;
        let line_bot = cmp::min(self.data_size.y - 1, line_top + self.size.y - 1);

        let diff_top = self.selected_line - line_top;
        let diff_bot = line_bot - self.selected_line;

        if diff_bot < scrolloff {
            self.scroll(0, scrolloff - diff_bot);
        } else if diff_top < scrolloff {
            self.scroll(0, diff_top - scrolloff);
        }
    }

    fn select(&mut self, d: i32) {
        let old_line = self.selected_line;

        self.selected_line += d;
        self.selected_line = cmp::min(self.selected_line, self.data_size.y - 1);
        self.selected_line = cmp::max(self.selected_line, 0);

        if self.selected_line != old_line {
            self.handle_scrolloff();
            self.dirty = true;
        }
    }

    fn select_first(&mut self) {
        if self.selected_line > 0 {
            self.selected_line = 0;
            self.handle_scrolloff();
            self.dirty = true;
        }
    }

    fn select_last(&mut self) {
        if self.selected_line < self.data_size.y - 1 {
            self.selected_line = self.data_size.y - 1;
            self.handle_scrolloff();
            self.dirty = true;
        }
    }

    fn toggle_expand(&mut self) {
        let id: usize = self.lookup[self.selected_line as usize];
        if self.expanded[id] {
            self.expanded[id] = false;
        } else {
            self.expanded[id] = true;
        }
        self.fetch_lines();

        self.dirty = true;
    }
}

impl<T: Tree> tui::Draw for TreeView<T> {
    fn draw(&mut self, rect: &tui::Rect, frame: &mut tui::Frame) {
        self.update_size(rect.max - rect.min + Point::new(1, 1));

        frame.clear_rect(rect);
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
                let p = Point::new(x as i32, y as i32);
                assert!(p.x <= rect.max.x);
                assert!(p.y <= rect.max.y);
                frame.add(tui::Cell::new(rect.min + p, c));
            }
        }

        for p in rect.points() {
            if p.y == self.selected_line - self.scroll.y {
                frame.cell_mut(p).bg = tui::LIGHT_GREY;
            }
        }

        self.dirty = false;
    }

    fn dirty(&self) -> bool {
        self.dirty
    }
}

impl<T: Tree> tui::Client for TreeView<T> {
    fn gen_lines(&mut self) -> Vec<String> {
        TVLineIter::new(self).collect()
    }

    fn handle_char(&mut self, c: char) {
        match c {
            ' ' => self.toggle_expand(),

            'h' => self.scroll(-self.size.x / 4, 0),
            'j' => self.select(1),
            'k' => self.select(-1),
            'l' => self.scroll(self.size.x / 4, 0),

            '0' => self.scroll_beg(),
            '$' => self.scroll_end(),
            'g' => self.select_first(),
            'G' => self.select_last(),

            'd' => self.select(self.size.y / 4),
            'u' => self.select(-self.size.y / 4),

            _ => (),
        }
    }
}

fn gen_path_prefix<T: Tree>(tree: &T, path: &[usize]) -> String {
    match path.len() {
        0 => panic!("Empty node path"),
        1...2 => "".to_string(),
        _ => path[1..path.len() - 1]
            .iter()
            .map(|&node| {
                if tree.next_sibling(node).is_some() {
                    "    │   "
                } else {
                    "        "
                }
            })
            .collect::<String>(),
    }
}

fn gen_line_prefix<T: Tree>(tv: &TreeView<T>, path: &[usize], is_first_line: bool) -> String {
    let last_id = *path.last().unwrap();
    let expand_marker = if tv.expanded[last_id] { "[+] " } else { "[-] " };
    match (
        path.len(),
        is_first_line,
        tv.tree.next_sibling(last_id).is_some(),
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
enum TVLineIterState {
    Node,
    Line,
}

#[derive(Debug)]
struct TVLineIter<'a, T: Tree> {
    state: TVLineIterState,
    tv: &'a mut TreeView<T>,

    node_iter: T::NodeIter,
    path: Vec<usize>,
    node_prefix: String,

    line_iter: Option<T::LineIter>,
    is_first_line: bool,
    line_prefix: String,
}

impl<'a, T: Tree> TVLineIter<'a, T> {
    fn new(tv: &'a mut TreeView<T>) -> Self {
        tv.lookup.clear();
        let node_iter = tv.tree.node_iter();
        TVLineIter {
            state: TVLineIterState::Node,
            tv,

            node_iter,
            path: Vec::new(),
            node_prefix: String::new(),

            line_iter: None,
            is_first_line: true,
            line_prefix: String::new(),
        }
    }
}

impl<'a, T: Tree> Iterator for TVLineIter<'a, T> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.state {
            TVLineIterState::Node => {
                loop {
                    self.path = self.node_iter.next()?;
                    if self
                        .path
                        .iter()
                        .rev()
                        .skip(1)
                        .all(|id| self.tv.expanded[*id])
                    {
                        break;
                    }
                }
                self.state = TVLineIterState::Line;
                self.node_prefix = gen_path_prefix(&self.tv.tree, &self.path);
                self.line_iter = Some(self.tv.tree.line_iter(*self.path.last().unwrap()));
                self.is_first_line = true;
                self.next()
            }
            TVLineIterState::Line => {
                if self.is_first_line {
                    self.line_prefix = gen_line_prefix(&self.tv, &self.path, true).to_string();
                }

                let res = {
                    if let Some(string) = self.line_iter.as_mut().unwrap().next() {
                        self.tv.lookup.push(*self.path.last().unwrap());
                        Some(format!(
                            "{}{}{}",
                            self.node_prefix, self.line_prefix, string
                        ))
                    } else {
                        self.state = TVLineIterState::Node;
                        return self.next();
                    }
                };

                if self.is_first_line {
                    self.is_first_line = false;
                    self.line_prefix = gen_line_prefix(&self.tv, &self.path, false).to_string();
                }

                res
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::process::tree::{NodeId, Tree, TreeIter};

    use std::collections::HashMap;

    struct ProcessMock {
        num_lines: u32,
        name: String,
    }

    impl ProcessMock {
        fn new<T: Into<String>>(num_lines: u32, name: T) -> Self {
            ProcessMock {
                num_lines,
                name: name.into(),
            }
        }
    }

    impl<'a> IntoIterator for &'a ProcessMock {
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

    impl<'a> super::Tree for &'a Tree<ProcessMock> {
        type NodeIter = TreeIter<'a, ProcessMock>;
        type LineIter = NodeIterMock;

        fn size(&self) -> usize {
            self.num_nodes()
        }
        fn next_sibling(&self, node: usize) -> Option<usize> {
            Tree::next_sibling(self, node)
        }

        fn node_iter(&self) -> Self::NodeIter {
            self.iter()
        }
        fn line_iter(&self, node: usize) -> Self::LineIter {
            let node = self.get(node).data();
            NodeIterMock {
                num_lines: node.num_lines,
                name: node.name.clone(),
            }
        }
    }

    fn make_tree(n: u32) -> (Tree<ProcessMock>, HashMap<String, NodeId>) {
        let mut t = Tree::new(ProcessMock {
            num_lines: n,
            name: "root".to_string(),
        });

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
            let id = t.insert(ProcessMock::new(n, name.to_string()), Some(parent));
            ids.insert(name.to_string(), id);
        }

        (t, ids)
    }

    #[test]
    fn iter() {
        let (t, ids) = make_tree(1);
        let mut tv = TreeView::new(&t);

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
        for (idx, line) in TVLineIter::new(&mut tv).enumerate() {
            line_count += 1;
            assert_eq!(line, expected_lines[idx]);
        }
        assert_eq!(line_count, expected_lines.len());
        for idx in 0..line_count {
            assert_eq!(tv.lookup[idx], expected_ids[idx]);
        }
    }

    #[test]
    fn iter_2lines() {
        let (t, ids) = make_tree(2);
        let mut tv = TreeView::new(&t);

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
        for (idx, line) in TVLineIter::new(&mut tv).enumerate() {
            line_count += 1;
            assert_eq!(line, expected_lines[idx]);
        }
        assert_eq!(line_count, expected_lines.len());
        for idx in 0..line_count {
            assert_eq!(tv.lookup[idx], expected_ids[idx]);
        }
    }
}
