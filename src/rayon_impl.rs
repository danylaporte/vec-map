use crate::VecMap;
use rayon::{
    iter::{
        IndexedParallelIterator, IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator,
    },
    slice::{Iter, IterMut},
};
use std::{
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

impl<K, V> VecMap<K, V>
where
    K: From<usize> + Send,
{
    pub fn par_iter(&self) -> ParIter<K, V>
    where
        V: Sync,
    {
        ParIter {
            _k: PhantomData,
            iter: self.vec.into_par_iter(),
        }
    }

    pub fn par_iter_mut(&mut self) -> ParIterMut<K, V>
    where
        V: Send,
    {
        ParIterMut {
            _k: PhantomData,
            iter: self.vec.par_iter_mut(),
        }
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// Same as [VecMap::retain] but in parallel.
    ///
    /// # Example
    /// ```
    /// use vec_map::VecMap;
    ///
    /// let mut map = VecMap::new();
    /// map.insert(1usize, 10);
    /// map.insert(2usize, 11);
    ///
    /// map.par_retain(|_k, v| v > &10);
    ///
    /// assert_eq!(map.len(), 1);
    /// assert_eq!(map.into_iter().collect::<Vec<_>>(), vec![(2, 11)]);
    /// ```
    pub fn par_retain<F>(&mut self, f: F)
    where
        F: Fn(&K, &V) -> bool + Sync,
        V: Send,
    {
        let len = AtomicUsize::new(self.len());

        self.vec
            .par_iter_mut()
            .enumerate()
            .for_each(|(index, item)| {
                if item.as_ref().map_or(false, |v| !f(&K::from(index), v)) {
                    *item = None;
                    len.fetch_sub(1, Relaxed);
                }
            });

        self.len = len.load(Relaxed);
    }
}

impl<'a, K: From<usize> + Send, V: Sync> IntoParallelIterator for &'a VecMap<K, V> {
    type Iter = ParIter<'a, K, V>;
    type Item = (K, &'a V);

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

impl<'a, K: From<usize> + Send, V: Send> IntoParallelIterator for &'a mut VecMap<K, V> {
    type Iter = ParIterMut<'a, K, V>;
    type Item = (K, &'a mut V);

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter_mut()
    }
}

pub struct ParIter<'a, K, V: Sync> {
    iter: Iter<'a, Option<V>>,
    _k: PhantomData<K>,
}

impl<'a, K: From<usize> + Send, V: Sync> ParallelIterator for ParIter<'a, K, V> {
    type Item = (K, &'a V);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        self.iter
            .enumerate()
            .filter_map(|(index, item)| Some((index, item.as_ref()?)))
            .map(|(index, item)| (index.into(), item))
            .drive_unindexed(consumer)
    }
}

pub struct ParIterMut<'a, K, V: Send> {
    iter: IterMut<'a, Option<V>>,
    _k: PhantomData<K>,
}

impl<'a, K, V> ParallelIterator for ParIterMut<'a, K, V>
where
    K: From<usize> + Send,
    V: Send,
{
    type Item = (K, &'a mut V);

    fn drive_unindexed<C>(self, consumer: C) -> C::Result
    where
        C: rayon::iter::plumbing::UnindexedConsumer<Self::Item>,
    {
        self.iter
            .enumerate()
            .filter_map(|(index, item)| Some((index, item.as_mut()?)))
            .map(|(index, item)| (index.into(), item))
            .drive_unindexed(consumer)
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
