// Hash set facade.
//
// The unordered hash set implementation lives in the collections module;
// this module re-exports it so programs can also refer to it as
// std::hash_set::HashSet.

pub use crate::collections::HashSet;
