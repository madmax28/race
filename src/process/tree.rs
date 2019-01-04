use crate::process::{ProcessData, ProcessDataLineIter};
use crate::tui::tv::Tree as TVTree;

pub type NodeId = usize;
pub type ProcessTree = Tree<ProcessData>;

#[derive(Debug)]
pub struct Node<T> {
    data: T,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
}

impl<T> Node<T> {
    fn new(data: T) -> Self {
        Node {
            data,
            parent: None,
            children: Vec::new(),
        }
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

#[derive(Debug)]
pub struct Tree<T> {
    nodes: Vec<Node<T>>,
}

impl<T> Tree<T> {
    pub fn new(root: T) -> Self {
        Tree {
            nodes: vec![Node::new(root)],
        }
    }

    pub fn insert(&mut self, data: T, parent_id: Option<NodeId>) -> NodeId {
        self.nodes.push(Node::new(data));

        let id = self.nodes.len() - 1;
        if let Some(parent_id) = parent_id {
            self.nodes[id].parent = Some(parent_id);
            self.nodes[parent_id].children.push(id);
        }

        id
    }

    pub fn get(&self, id: NodeId) -> &Node<T> {
        &self.nodes[id]
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut Node<T> {
        &mut self.nodes[id]
    }

    pub fn set_parent(&mut self, id: NodeId, parent_id: NodeId) {
        assert!(self.nodes[id].parent.is_none());

        self.nodes[id].parent = Some(parent_id);
        self.nodes[parent_id].children.push(id);
    }

    pub fn next_sibling(&self, id: NodeId) -> Option<NodeId> {
        let parent_id = self.nodes[id].parent?;
        let mut iter = self.nodes[parent_id].children.iter();
        iter.find(|i| **i == id)?;
        Some(*iter.next()?)
    }

    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub fn iter(&self) -> TreeIter<T> {
        TreeIter::new(&self)
    }
}

#[derive(Debug)]
pub struct TreeIter<'a, T: 'a> {
    tree: &'a Tree<T>,
    frontier: Vec<Vec<NodeId>>,
}

impl<'a, T: 'a> TreeIter<'a, T> {
    fn new(tree: &'a Tree<T>) -> Self {
        TreeIter {
            tree,
            frontier: vec![vec![0]],
        }
    }
}

impl<'a, T> Iterator for TreeIter<'a, T> {
    type Item = Vec<NodeId>;

    fn next(&mut self) -> Option<Self::Item> {
        let path = self.frontier.pop()?;

        {
            let node = &self.tree.get(*path.last().unwrap());
            for child_idx in node.children.iter().rev() {
                let mut p = path.clone();
                p.push(*child_idx);
                self.frontier.push(p);
            }
        }

        Some(path)
    }
}

impl<'a> TVTree for &'a Tree<ProcessData> {
    type NodeIter = TreeIter<'a, ProcessData>;
    type LineIter = ProcessDataLineIter<'a>;

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
        ProcessDataLineIter::new(self.get(node).data())
    }
}
