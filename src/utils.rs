use std::collections::HashSet;

pub fn union_strings(a: Vec<String>, b: Vec<String>) -> Vec<String> {
    let a_set: HashSet<_> = HashSet::from_iter(a);
    let b_set: HashSet<_> = HashSet::from_iter(b);

    a_set.union(&b_set).cloned().collect::<Vec<String>>()
}
