#[cfg(feature = "rayon")]
mod rayon_impl;

#[cfg(feature = "rayon")]
pub use rayon_impl::*;

use std::{
    fmt::{self, Debug},
    iter::{Enumerate, FromIterator},
    marker::PhantomData,
    mem::replace,
};

pub struct VecMap<K, V> {
    _k: PhantomData<K>,
    len: usize,
    vec: Vec<Option<V>>,
}

impl<K, V> VecMap<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self {
            _k: PhantomData,
            len: 0,
            vec: Vec::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            _k: PhantomData,
            len: 0,
            vec: Vec::with_capacity(capacity),
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.vec.clear();
    }

    pub fn contains_key(&self, key: &K) -> bool
    where
        K: Clone + Into<usize>,
    {
        self.vec.get(index(key)).map_or(false, |o| o.is_some())
    }

    fn ensure_index(&mut self, index: usize) {
        let iter = (self.vec.len()..=index).into_iter().map(|_| None);
        self.vec.extend(iter);
    }

    pub fn entry(&mut self, key: K) -> Entry<K, V>
    where
        K: Clone + Into<usize>,
    {
        if self.contains_key(&key) {
            Entry::Occupied(OccupiedEntry { key, vec: self })
        } else {
            Entry::Vacant(VacantEntry { key, vec: self })
        }
    }

    #[inline]
    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: Clone + Into<usize>,
    {
        self.vec.get(index(key)).and_then(|v| v.as_ref())
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V>
    where
        K: Clone + Into<usize>,
    {
        self.vec.get_mut(index(key)).and_then(|v| v.as_mut())
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Into<usize>,
    {
        let index: usize = key.into();
        self.ensure_index(index);

        let out = replace(unsafe { self.vec.get_unchecked_mut(index) }, Some(value));

        if out.is_none() {
            self.len += 1;
        }

        out
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            _k: PhantomData,
            it: self.vec.iter().enumerate(),
            len: self.len,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            _k: PhantomData,
            it: self.vec.iter_mut().enumerate(),
            len: self.len,
        }
    }

    pub fn keys(&self) -> Keys<K, V> {
        Keys(self.iter())
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn remove(&mut self, key: &K) -> Option<V>
    where
        K: Clone + Into<usize>,
    {
        let out = self.vec.get_mut(index(key)).and_then(|v| v.take());

        if out.is_some() {
            self.len -= 1;
        }

        out
    }

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all elements e such that f(&e) returns false. This method operates in place,
    /// visiting each element exactly once in the original order, and preserves the order of the retained elements.
    ///
    /// # Example
    /// ```
    /// use vec_map::VecMap;
    ///
    /// let mut map = VecMap::new();
    /// map.insert(1usize, 10);
    /// map.insert(2usize, 11);
    ///
    /// map.retain(|_k, v| v > &10);
    ///
    /// assert_eq!(map.len(), 1);
    /// assert_eq!(map.into_iter().collect::<Vec<_>>(), vec![(2, 11)]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &V) -> bool,
        K: From<usize>,
    {
        let len = &mut self.len;

        self.vec.iter_mut().enumerate().for_each(|(index, item)| {
            if item.as_ref().map_or(false, |v| !f(&K::from(index), v)) {
                *item = None;
                *len -= 1;
            }
        });
    }

    pub fn shrink_to_fit(&mut self) {
        if let Some(index) = self
            .vec
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_some())
            .map(|(i, _)| i)
            .last()
        {
            self.vec.drain(index..);
            self.shrink_to_fit();
        }
    }

    pub fn values(&self) -> Values<K, V> {
        Values(self.iter())
    }

    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut(self.iter_mut())
    }
}

impl<K, V> Clone for VecMap<K, V>
where
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            _k: PhantomData,
            len: self.len,
            vec: self.vec.clone(),
        }
    }
}

impl<K, V> Default for VecMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Extend<(K, V)> for VecMap<K, V>
where
    K: Into<usize>,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<K, V> FromIterator<(K, V)> for VecMap<K, V>
where
    K: Into<usize>,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let capacity = iter.size_hint().0;

        iter.fold(Self::with_capacity(capacity), |mut vec, (k, v)| {
            vec.insert(k, v);
            vec
        })
    }
}

impl<K, V> IntoIterator for VecMap<K, V>
where
    K: From<usize>,
{
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            _k: PhantomData,
            it: self.vec.into_iter().enumerate(),
            len: self.len,
        }
    }
}

impl<'a, K, V> IntoIterator for &'a VecMap<K, V>
where
    K: From<usize>,
{
    type Item = (K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut VecMap<K, V>
where
    K: From<usize>,
{
    type Item = (K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V> Eq for VecMap<K, V> where V: Eq + PartialEq {}

impl<K, V> PartialEq for VecMap<K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        fn iter<T>(vec: &[Option<T>]) -> impl Iterator<Item = (usize, &Option<T>)> {
            vec.iter().enumerate().filter(|(_, v)| v.is_some())
        }

        iter(&self.vec).eq(iter(&other.vec))
    }
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Clone + Into<usize>,
    {
        match self {
            Self::Occupied(o) => o.into_mut(),
            Self::Vacant(v) => v.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    ///
    /// # Examples
    ///
    /// ```
    /// use vec_map::VecMap;
    ///
    /// let mut map: VecMap<usize, String> = VecMap::new();
    /// let s = "hoho".to_string();
    ///
    /// map.entry(2).or_insert_with(|| s);
    ///
    /// assert_eq!(map.get(&2).unwrap().clone(), "hoho".to_string());
    /// ```
    pub fn or_insert_with<F>(self, default: F) -> &'a mut V
    where
        F: FnOnce() -> V,
        K: Clone + Into<usize>,
    {
        match self {
            Self::Occupied(o) => o.into_mut(),
            Self::Vacant(v) => v.insert(default()),
        }
    }

    pub fn key(&self) -> &K {
        match self {
            Self::Occupied(o) => o.key(),
            Self::Vacant(v) => v.key(),
        }
    }

    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
        K: Clone + Into<usize>,
    {
        match self {
            Self::Occupied(mut o) => {
                f(o.get_mut());
                Self::Occupied(o)
            }
            Self::Vacant(v) => Self::Vacant(v),
        }
    }

    pub fn or_default(self) -> &'a mut V
    where
        K: Clone + Into<usize>,
        V: Default,
    {
        match self {
            Self::Occupied(o) => o.into_mut(),
            Self::Vacant(v) => v.insert(Default::default()),
        }
    }
}

pub struct IntoIter<K, V> {
    _k: PhantomData<K>,
    it: Enumerate<std::vec::IntoIter<Option<V>>>,
    len: usize,
}

impl<K, V> DoubleEndedIterator for IntoIter<K, V>
where
    K: From<usize>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some((index, item)) = self.it.next_back() {
            if let Some(v) = item {
                return Some((index.into(), v));
            }
        }

        None
    }
}

impl<K, V> Iterator for IntoIter<K, V>
where
    K: From<usize>,
{
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((index, opt)) = self.it.next() {
            if let Some(v) = opt {
                return Some((index.into(), v));
            }
        }

        None
    }

    fn count(self) -> usize {
        self.len
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

pub struct Iter<'a, K, V> {
    _k: PhantomData<K>,
    it: Enumerate<std::slice::Iter<'a, Option<V>>>,
    len: usize,
}

impl<'a, K, V> Clone for Iter<'a, K, V> {
    fn clone(&self) -> Self {
        Self {
            _k: self._k,
            it: self.it.clone(),
            len: self.len,
        }
    }
}

impl<'a, K, V> DoubleEndedIterator for Iter<'a, K, V>
where
    K: From<usize>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some((index, item)) = self.it.next_back() {
            if let Some(v) = item {
                return Some((index.into(), v));
            }
        }

        None
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: From<usize>,
{
    type Item = (K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((index, opt)) = self.it.next() {
            if let Some(v) = opt {
                return Some((index.into(), v));
            }
        }

        None
    }

    #[inline]
    fn count(self) -> usize {
        self.len
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

pub struct IterMut<'a, K, V> {
    _k: PhantomData<K>,
    it: Enumerate<std::slice::IterMut<'a, Option<V>>>,
    len: usize,
}

impl<'a, K, V> DoubleEndedIterator for IterMut<'a, K, V>
where
    K: From<usize>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        while let Some((index, item)) = self.it.next_back() {
            if let Some(v) = item {
                return Some((index.into(), v));
            }
        }

        None
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
where
    K: From<usize>,
{
    type Item = (K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        for (index, item) in self.it.by_ref() {
            if let Some(v) = item {
                return Some((index.into(), v));
            }
        }

        None
    }

    fn count(self) -> usize {
        self.len
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

pub struct Keys<'a, K, V>(Iter<'a, K, V>);

impl<'a, K, V> Clone for Keys<'a, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a, K, V> DoubleEndedIterator for Keys<'a, K, V>
where
    K: From<usize>,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, _)| k)
    }
}

impl<'a, K, V> Iterator for Keys<'a, K, V>
where
    K: From<usize>,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    fn count(self) -> usize {
        self.0.count()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct OccupiedEntry<'a, K, V> {
    key: K,
    vec: &'a mut VecMap<K, V>,
}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub fn get(&self) -> &V
    where
        K: Clone + Into<usize>,
    {
        self.vec.get(&self.key).unwrap()
    }

    pub fn get_mut(&mut self) -> &mut V
    where
        K: Clone + Into<usize>,
    {
        self.vec.get_mut(&self.key).unwrap()
    }

    pub fn insert(&mut self, value: V) -> V
    where
        K: Clone + Into<usize>,
    {
        self.vec.insert(self.key.clone(), value).unwrap()
    }

    pub fn into_mut(self) -> &'a mut V
    where
        K: Clone + Into<usize>,
    {
        self.vec.get_mut(&self.key).unwrap()
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn remove(self) -> V
    where
        K: Clone + Into<usize>,
    {
        self.vec.remove(&self.key).unwrap()
    }

    pub fn remove_entry(self) -> (K, V)
    where
        K: Clone + Into<usize>,
    {
        let v = self.vec.remove(&self.key).unwrap();
        (self.key, v)
    }
}

impl<K: Debug, V> Debug for OccupiedEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("OccupiedEntry").field(self.key()).finish()
    }
}

pub struct VacantEntry<'a, K, V> {
    key: K,
    vec: &'a mut VecMap<K, V>,
}

impl<'a, K, V> VacantEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn into_key(self) -> K {
        self.key
    }

    pub fn insert(self, value: V) -> &'a mut V
    where
        K: Clone + Into<usize>,
    {
        self.vec.insert(self.key.clone(), value);
        self.vec.get_mut(&self.key).unwrap()
    }
}

impl<K: Debug, V> Debug for VacantEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}

pub struct Values<'a, K, V>(Iter<'a, K, V>);

impl<'a, K, V> Clone for Values<'a, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a, K, V> DoubleEndedIterator for Values<'a, K, V>
where
    K: From<usize>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, v)| v)
    }
}

impl<'a, K, V> Iterator for Values<'a, K, V>
where
    K: From<usize>,
{
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    fn count(self) -> usize {
        self.0.count()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ValuesMut<'a, K, V>(IterMut<'a, K, V>);

impl<'a, K, V> DoubleEndedIterator for ValuesMut<'a, K, V>
where
    K: From<usize>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, v)| v)
    }
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V>
where
    K: From<usize>,
{
    type Item = &'a mut V;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    fn count(self) -> usize {
        self.0.count()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

fn index<K>(key: &K) -> usize
where
    K: Clone + Into<usize>,
{
    key.clone().into()
}

#[test]
fn test_insert() {
    let mut vec = VecMap::new();

    for n in (0..30usize).rev() {
        assert!(vec.insert(n, n).is_none());
    }

    assert_eq!(vec.len(), 30);

    for n in (0..30usize).rev() {
        let old = vec.insert(n, 100 - n);

        assert_eq!(n, old.unwrap());
    }

    assert_eq!(vec.len(), 30);
}
