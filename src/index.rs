use crate::doc::Doc;

pub struct Index {
    pub docs: Vec<Doc>,
}

impl Index {
    pub fn new() -> Index {
        Index { docs: Vec::new() }
    }

    pub fn insert(&mut self, doc: Doc) {
        self.docs.push(doc);
    }
}

impl Default for Index {
    fn default() -> Self {
        Self::new()
    }
}
