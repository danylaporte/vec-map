use crate::VecMap;
use rayon::{
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
    slice::Iter,
};
use std::marker::PhantomData;

impl<K, V> VecMap<K, V>
where
    K: From<usize> + Send,
    V: Sync,
{
    pub fn par_iter(&self) -> ParIter<K, V> {
        ParIter {
            _k: PhantomData,
            iter: self.vec.par_iter(),
        }
    }
}

impl<'a, K: From<usize> + Send + 'a, V: Sync + 'a> IntoParallelRefIterator<'a> for VecMap<K, V> {
    type Iter = ParIter<'a, K, V>;
    type Item = (K, &'a V);

    fn par_iter(&'a self) -> Self::Iter {
        self.par_iter()
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

#[test]
fn test_rayon() {
    use std::ops::Rem;

    let vm = (0..1000)
        .into_iter()
        .map(|i| (i, i))
        .collect::<VecMap<usize, usize>>();

    let count = vm.par_iter().filter(|(k, _)| k.rem(2) == 0).count();
    assert_eq!(count, 500);
}
