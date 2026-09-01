use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    marker::PhantomData,
    ops::Deref,
    sync::{
        self, LazyLock, RwLock, RwLockReadGuard,
        atomic::{AtomicUsize, Ordering},
    },
};

use dashmap::{
    DashMap,
    mapref::{
        multiple::RefMulti,
        one::{Ref, RefMut},
    },
};

/* --------------------------------- STRUCTS -------------------------------- */

pub struct Interned<T: Hash + Eq> {
    element_to_handle: LazyLock<DashMap<T, Handle<T>>>,
    handle_to_element: LazyLock<DashMap<Handle<T>, T>>,

    next_id: AtomicUsize,
}

pub struct Handle<T>(pub(crate) usize, PhantomData<T>);

/* ---------------------------------- IMPLS --------------------------------- */

impl<T: Hash + Eq> Interned<T>
where
    Handle<T>: Eq + Hash,
    T: Clone,
{
    pub const fn new() -> Self {
        Self {
            element_to_handle: LazyLock::new(|| DashMap::new()),
            handle_to_element: LazyLock::new(|| DashMap::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    pub fn insert(&self, val: T) -> Handle<T> {
        let id =
            Handle(self.next_id.fetch_add(1, Ordering::Relaxed), PhantomData);

        self.handle_to_element.insert(id, val.clone());
        self.element_to_handle.insert(val, id);

        id
    }

    pub(crate) fn insert_at(&self, id: usize, val: T) {
        let handle = Handle::new(id);

        self.handle_to_element.insert(handle, val.clone());
        self.element_to_handle.insert(val, handle);

        self.next_id.fetch_max(id + 1, Ordering::Relaxed);
    }

    pub fn get_cloned(&self, id: Handle<T>) -> Option<T> {
        self.handle_to_element.get(&id).map(|x| x.clone())
    }

    pub fn get(&self, id: Handle<T>) -> Option<Ref<'_, Handle<T>, T>> {
        self.handle_to_element.get(&id)
    }

    pub fn handle_of(&self, val: &T) -> Option<Handle<T>> {
        self.element_to_handle.get(val).map(|x| x.clone())
    }

    pub fn modify(
        &self,
        id: Handle<T>,
        f: impl FnOnce(RefMut<'_, Handle<T>, T>) -> (),
    ) {
        self.handle_to_element.get_mut(&id).map(f);
    }

    pub fn find(
        &self,
        mut f: impl FnMut(Handle<T>, &T) -> bool,
    ) -> Option<(Handle<T>, T)> {
        self.handle_to_element
            .iter()
            .find(|r| f(*r.key(), r.value()))
            .map(|r| (*r.key(), r.value().clone()))
    }
}

impl<T> Handle<T> {
    pub(crate) const fn new(id: usize) -> Self {
        Self(id, PhantomData)
    }
}

impl<T> Copy for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Id").field(&self.0).finish()
    }
}
