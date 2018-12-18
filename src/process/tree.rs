use crate::process::{ProcessData, ProcessDataLineIter};
use crate::tree::{Tree, TreeIter};
use crate::tui::tv::Tree as TVTree;

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
