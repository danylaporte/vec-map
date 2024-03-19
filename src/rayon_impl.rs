use crate::VecMap;
use rayon::{
    iter::{
        IntoParallelIterator, IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator,
    },
    slice::{Iter, IterMut},
};

impl<K, V> VecMap<K, V> {
    pub fn par_iter(&self) -> ParIter<K, V>
    where
        K: Sync,
        V: Sync,
    {
        ParIter(self.rows.par_iter())
    }

    pub fn par_iter_mut(&mut self) -> ParIterMut<K, V>
    where
        K: Send,
        V: Send,
    {
        ParIterMut(self.rows.par_iter_mut())
    }
}

impl<'a, K: Copy + Send + Sync, V: Sync> IntoParallelIterator for &'a VecMap<K, V> {
    type Iter = ParIter<'a, K, V>;
    type Item = (K, &'a V);

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

impl<'a, K: Copy + Send, V: Send> IntoParallelIterator for &'a mut VecMap<K, V> {
    type Iter = ParIterMut<'a, K, V>;
    type Item = (K, &'a mut V);

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter_mut()
    }
}

pub struct ParIter<'a, K: Sync, V: Sync>(Iter<'a, (K, V)>);

impl<'a, K: Copy + Send + Sync, V: Sync> ParallelIterator for ParIter<'a, K, V> {
    type Item = (K, &'a V);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        self.0.map(|t| (t.0, &t.1)).drive_unindexed(consumer)
    }
}

pub struct ParIterMut<'a, K: Send, V: Send>(IterMut<'a, (K, V)>);

impl<'a, K, V> ParallelIterator for ParIterMut<'a, K, V>
where
    K: Copy + Send,
    V: Send,
{
    type Item = (K, &'a mut V);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        self.0.map(|t| (t.0, &mut t.1)).drive_unindexed(consumer)
    }
}

#[test]
fn test_rayon() {
    use std::ops::Rem;

    let vm = (0..1000)
        .into_iter()
        .map(|i| (i, i))
        .collect::<VecMap<usize, usize>>();

    let count = vm.par_iter().filter(|(k, _)| k.rem(2) == 0).count();
    assert_eq!(count, 500);

    let count = (&vm).into_par_iter().count();
    assert_eq!(count, 1000);
}

#[test]
fn test_rayon_mut() {
    let mut vm = (0..1000)
        .into_iter()
        .map(|i| (i, i))
        .collect::<VecMap<usize, usize>>();

    vm.par_iter_mut().for_each(|(_, v)| *v = *v * 2);

    (&mut vm).into_par_iter().for_each(|(_, v)| *v = *v + 1);
}
