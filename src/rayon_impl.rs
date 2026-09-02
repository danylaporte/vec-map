use crate::VecMap;
use rayon::{
    iter::{
        IndexedParallelIterator, IntoParallelIterator, IntoParallelRefIterator,
        IntoParallelRefMutIterator, ParallelIterator,
        plumbing::{Consumer, ProducerCallback},
    },
    slice::{Iter, IterMut},
};

impl<K, V> VecMap<K, V> {
    pub fn par_iter(&self) -> ParIter<'_, K, V>
    where
        K: Sync,
        V: Sync,
    {
        ParIter(self.rows.par_iter())
    }

    pub fn par_iter_mut(&mut self) -> ParIterMut<'_, K, V>
    where
        K: Send,
        V: Send,
    {
        ParIterMut(self.rows.par_iter_mut())
    }
}

impl<'a, K: Sync, V: Sync> IntoParallelIterator for &'a VecMap<K, V> {
    type Iter = ParIter<'a, K, V>;
    type Item = (&'a K, &'a V);

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

impl<'a, K: Send + Sync, V: Send> IntoParallelIterator for &'a mut VecMap<K, V> {
    type Iter = ParIterMut<'a, K, V>;
    type Item = (&'a K, &'a mut V);

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter_mut()
    }
}

pub struct ParIter<'a, K: Sync, V: Sync>(Iter<'a, (K, V)>);

impl<'a, K: Sync, V: Sync> ParallelIterator for ParIter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        self.0.map(|t| (&t.0, &t.1)).drive_unindexed(consumer)
    }
}

impl<'a, K: Sync, V: Sync> IndexedParallelIterator for ParIter<'a, K, V> {
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        self.0.map(|t| (&t.0, &t.1)).drive(consumer)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        self.0.map(|t| (&t.0, &t.1)).with_producer(callback)
    }
}

pub struct ParIterMut<'a, K: Send, V: Send>(IterMut<'a, (K, V)>);

impl<'a, K, V> ParallelIterator for ParIterMut<'a, K, V>
where
    K: Send + Sync,
    V: Send,
{
    type Item = (&'a K, &'a mut V);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        self.0.map(|t| (&t.0, &mut t.1)).drive_unindexed(consumer)
    }
}

impl<'a, K, V> IndexedParallelIterator for ParIterMut<'a, K, V>
where
    K: Send + Sync,
    V: Send,
{
    fn drive<C>(self, consumer: C) -> C::Result
    where
        C: Consumer<Self::Item>,
    {
        self.0.map(|t| (&t.0, &mut t.1)).drive(consumer)
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn with_producer<CB>(self, callback: CB) -> CB::Output
    where
        CB: ProducerCallback<Self::Item>,
    {
        self.0.map(|t| (&t.0, &mut t.1)).with_producer(callback)
    }
}

#[test]
fn test_rayon() {
    use std::ops::Rem;

    let vm = (0..1000)
        .into_iter()
        .map(|i| (i, i))
        .collect::<VecMap<u32, u32>>();

    let count = vm.par_iter().filter(|(k, _)| k.rem(2) == 0).count();
    assert_eq!(count, 500);

    let count = (&vm).into_par_iter().count();
    assert_eq!(count, 1000);

    let indexed = vm
        .par_iter()
        .enumerate()
        .zip((0..1000u32).into_par_iter())
        .all(|((index, (key, value)), expected)| {
            index == expected as usize && *key == expected && *value == expected
        });
    assert!(indexed);
}

#[test]
fn test_rayon_mut() {
    let mut vm = (0..1000)
        .into_iter()
        .map(|i| (i, i))
        .collect::<VecMap<u32, u32>>();

    vm.par_iter_mut().for_each(|(_, v)| *v *= 2);

    (&mut vm).into_par_iter().for_each(|(_, v)| *v += 1);

    vm.par_iter_mut()
        .enumerate()
        .for_each(|(index, (_, value))| *value = index as u32);

    assert!(
        vm.iter()
            .enumerate()
            .all(|(index, (_, value))| { *value == index as u32 })
    );
}
