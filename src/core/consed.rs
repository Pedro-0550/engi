use core::slice;
use std::{
    hash::Hash,
    ops::{Deref, DerefMut},
    vec,
};

use ahash::AHashMap;

use crate::core::index::Indexed;

pub enum Insertion<Key, Idx> {
    Existed(Key),
    Added(Idx),
}

/// Hash-consing Vec<T>
#[derive(PartialEq, Eq, Clone)]
pub struct Consed<T: Hash + Eq, Key, Idx = Key> {
    cons: AHashMap<T, Key>,
    values: Indexed<Vec<T>, Idx>,
}

impl<T, Key> Consed<T, Key, Key>
where
    T: Hash + Eq + Clone,
    Key: From<usize> + Copy,
{
    pub fn insert(&mut self, val: T) -> Key {
        if let Some(existing) = self.cons.get(&val) {
            *existing
        } else {
            let idx = self.values.len();
            let key = Key::from(idx as usize);

            self.values.push(val.clone());
            self.cons.insert(val, key);

            key
        }
    }
}

impl<T, Key, Idx> Consed<T, Key, Idx>
where
    T: Hash + Eq + Clone,
    Idx: From<usize>,
    Key: Copy,
{
    pub fn contains(&self, val: &T) -> bool {
        self.cons.contains_key(val)
    }

    pub fn insert_with(&mut self, val: T, key: Key) -> Insertion<Key, Idx> {
        if let Some(existing) = self.cons.get(&val) {
            Insertion::Existed(*existing)
        } else {
            let idx = self.values.next();
            self.values.push(val.clone());
            self.cons.insert(val, key);

            Insertion::Added(idx)
        }
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Idx, &T)>
    where
        T: 'static, {
        self.values.iter()
    }
}

impl<T, Key, Idx> Default for Consed<T, Key, Idx>
where
    T: Hash + Eq,
{
    fn default() -> Self {
        Self { cons: Default::default(), values: Vec::new().into() }
    }
}

impl<T, Key, Idx> DerefMut for Consed<T, Key, Idx>
where
    T: Hash + Eq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}

impl<T, Key, Idx> Deref for Consed<T, Key, Idx>
where
    T: Hash + Eq,
{
    type Target = Indexed<Vec<T>, Idx>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
