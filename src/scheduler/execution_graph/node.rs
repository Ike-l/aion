use std::{collections::HashSet, hash::Hash, rc::Rc};

pub struct Node<T> {
    pub parent: Option<Rc<Self>>, 
    pub id: T
}

impl<T> Node<T> where T: Eq + Hash + Clone {
    pub fn get_lineage(&self) -> HashSet<T> {
        let mut lineage = HashSet::new();

        let mut current = self;
        while current.parent.is_some() {
            lineage.insert(current.id.clone());
            current = current.parent.as_ref().unwrap();
        }

        lineage.insert(current.id.clone());

        lineage
    }
}