use std::{
    marker::PhantomData,
    ops::{self, Deref, DerefMut, Index, IndexMut},
};

use crate::expr::dag;

#[repr(transparent)]
#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Indexed<T, I> {
    collection: T,
    _phantom: PhantomData<I>,
}

impl<T, I> Indexed<T, I> {
    pub fn iter<U>(&self) -> impl Iterator<Item = (I, &U)>
    where
        I: From<usize>,
        T: AsRef<[U]>,
        U: 'static, {
        self.collection
            .as_ref()
            .iter()
            .enumerate()
            .map(|(i, t)| (I::from(i), t))
    }

    pub fn get<U>(&self, idx: I) -> Option<&U>
    where
        T: AsRef<[U]>,
        I: Into<usize>, {
        self.collection.as_ref().get(idx.into())
    }

    pub fn get_mut<U>(&mut self, idx: I) -> Option<&mut U>
    where
        T: AsMut<[U]>,
        I: Into<usize>, {
        self.collection.as_mut().get_mut(idx.into())
    }
}

// impl<T, I> DerefMut for Indexed<T, I> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.collection
//     }
// }

// impl<T, I> Deref for Indexed<T, I> {
//     type Target = T;

//     fn deref(&self) -> &Self::Target {
//         &self.collection
//     }
// }

impl<T, I> Index<I> for Indexed<T, I>
where
    T: Index<usize>,
    I: Into<usize>,
{
    type Output = <T as Index<usize>>::Output;

    fn index(&self, index: I) -> &Self::Output {
        &self.collection[index.into()]
    }
}

impl<T, I> IndexMut<I> for Indexed<T, I>
where
    T: IndexMut<usize>,
    I: Into<usize>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        &mut self.collection[index.into()]
    }
}

impl<T, I> From<T> for Indexed<T, I> {
    fn from(value: T) -> Self {
        Self { collection: value, _phantom: PhantomData }
    }
}
