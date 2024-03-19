#[cfg(feature = "rayon")]
mod rayon_impl;

#[cfg(feature = "rayon")]
pub use rayon_impl::*;

use std::{
    fmt::{self, Debug},
    iter::FromIterator,
    mem::replace,
};

pub struct VecMap<K, V> {
    keys: Vec<Option<u32>>,
    rows: Vec<(K, V)>,
}

impl<K, V> VecMap<K, V> {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keys: Vec::new(),
            rows: Vec::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity),
            rows: Vec::with_capacity(capacity),
        }
    }

    pub fn clear(&mut self) {
        self.keys.clear();
        self.rows.clear();
    }
    
    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool
    where
        K: Copy + Into<usize>,
    {
        self.keys.get(index(key)).map_or(false, Option::is_some)
    }

    #[must_use]
    pub fn entry(&mut self, key: K) -> Entry<K, V>
    where
        K: Copy + Into<usize>,
    {
        if self.contains_key(&key) {
            Entry::Occupied(OccupiedEntry { key, vec: self })
        } else {
            Entry::Vacant(VacantEntry { key, vec: self })
        }
    }

    #[inline]
    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V>
    where
        K: Copy + Into<usize>,
    {
        match self.keys.get(index(key)) {
            Some(Some(index)) => unsafe { Some(&self.rows.get_unchecked(*index as usize).1) },
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V>
    where
        K: Copy + Into<usize>,
    {
        match self.keys.get_mut(index(key)) {
            Some(Some(index)) => unsafe {
                Some(&mut self.rows.get_unchecked_mut(*index as usize).1)
            },
            _ => None,
        }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Copy + Into<usize>,
    {
        let index: usize = key.into();

        let index = match self.keys.get_mut(index) {
            Some(key) => key,
            None => {
                self.keys.extend((self.keys.len()..=index).map(|_| None));

                unsafe { self.keys.get_unchecked_mut(index) }
            }
        };

        match index {
            &mut Some(index) => Some(replace(
                &mut unsafe { self.rows.get_unchecked_mut(index as usize) }.1,
                value,
            )),
            None => {
                *index = Some(self.rows.len() as u32);
                self.rows.push((key, value));
                None
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> Iter<K, V> {
        Iter(self.rows.iter())
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut(self.rows.iter_mut())
    }

    #[inline]
    pub fn keys(&self) -> Keys<K, V> {
        Keys(self.iter())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn remove(&mut self, key: &K) -> Option<V>
    where
        K: Copy + Into<usize>,
    {
        if let Some(row_index) = self
            .keys
            .get_mut(index(key))
            .and_then(Option::take)
            .map(|i| i as usize)
        {
            if self.rows.len() - 1 != row_index {
                if let Some(k) = self.rows.last().map(|t| index(&t.0)) {
                    *self.keys.get_mut(k).expect("key") = Some(row_index as u32);
                }
            }

            Some(self.rows.swap_remove(row_index).1)
        } else {
            None
        }
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
        K: Copy + From<usize>,
    {
        let mut count = 0;

        self.rows.retain(|t| {
            let retain = f(&t.0, &t.1);

            if !retain {
                self.keys.iter_mut().for_each(|o| match *o {
                    Some(index) if index == count => *o = None,
                    Some(index) if index > count => *o = Some(index - 1),
                    _ => {}
                });
            }

            count += 1;

            retain
        })
    }

    pub fn shrink_to_fit(&mut self) {
        if let Some(index) = self
            .keys
            .iter()
            .enumerate()
            .filter(|t| t.1.is_some())
            .map(|t| t.0)
            .last()
        {
            self.keys.drain(index..);
        }

        self.keys.shrink_to_fit();
        self.rows.shrink_to_fit();
    }

    pub fn values(&self) -> Values<K, V> {
        Values(self.rows.iter())
    }

    pub fn values_mut(&mut self) -> ValuesMut<K, V> {
        ValuesMut(self.rows.iter_mut())
    }
}

impl<K, V> Clone for VecMap<K, V>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            rows: self.rows.clone(),
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
    K: Copy + Into<usize>,
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
    K: Copy + Into<usize>,
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

impl<K, V> IntoIterator for VecMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.rows.into_iter())
    }
}

impl<'a, K, V> IntoIterator for &'a VecMap<K, V>
where
    K: Copy,
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
    K: Copy,
{
    type Item = (K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V> Eq for VecMap<K, V>
where
    K: Eq + PartialEq,
    V: Eq + PartialEq,
{
}

impl<K, V> PartialEq for VecMap<K, V>
where
    K: PartialEq,
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.rows == other.rows
    }
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Copy + Into<usize>,
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
        K: Copy + Into<usize>,
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
        K: Copy + Into<usize>,
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
        K: Copy + Into<usize>,
        V: Default,
    {
        match self {
            Self::Occupied(o) => o.into_mut(),
            Self::Vacant(v) => v.insert(Default::default()),
        }
    }
}

pub struct IntoIter<K, V>(std::vec::IntoIter<(K, V)>);

impl<K, V> DoubleEndedIterator for IntoIter<K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl<K, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct Iter<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<'a, K, V> Clone for Iter<'a, K, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a, K, V> DoubleEndedIterator for Iter<'a, K, V>
where
    K: Copy,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|t| (t.0, &t.1))
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: Copy,
{
    type Item = (K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|t| (t.0, &t.1))
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct IterMut<'a, K, V>(std::slice::IterMut<'a, (K, V)>);

impl<'a, K, V> DoubleEndedIterator for IterMut<'a, K, V>
where
    K: Copy,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|t| (t.0, &mut t.1))
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V>
where
    K: Copy,
{
    type Item = (K, &'a mut V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|t| (t.0, &mut t.1))
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct Keys<'a, K, V>(Iter<'a, K, V>);

impl<'a, K, V> Clone for Keys<'a, K, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a, K, V> DoubleEndedIterator for Keys<'a, K, V>
where
    K: Copy,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, _)| k)
    }
}

impl<'a, K, V> Iterator for Keys<'a, K, V>
where
    K: Copy,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
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
        K: Copy + Into<usize>,
    {
        self.vec.get(&self.key).unwrap()
    }

    pub fn get_mut(&mut self) -> &mut V
    where
        K: Copy + Into<usize>,
    {
        self.vec.get_mut(&self.key).unwrap()
    }

    pub fn insert(&mut self, value: V) -> V
    where
        K: Copy + Into<usize>,
    {
        self.vec.insert(self.key, value).unwrap()
    }

    pub fn into_mut(self) -> &'a mut V
    where
        K: Copy + Into<usize>,
    {
        self.vec.get_mut(&self.key).unwrap()
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn remove(self) -> V
    where
        K: Copy + Into<usize>,
    {
        self.vec.remove(&self.key).unwrap()
    }

    pub fn remove_entry(self) -> (K, V)
    where
        K: Copy + Into<usize>,
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
        K: Copy + Into<usize>,
    {
        self.vec.insert(self.key, value);
        self.vec.get_mut(&self.key).unwrap()
    }
}

impl<K: Debug, V> Debug for VacantEntry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}

pub struct Values<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<'a, K, V> Clone for Values<'a, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<'a, K, V> DoubleEndedIterator for Values<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, v)| v)
    }
}

impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

pub struct ValuesMut<'a, K, V>(std::slice::IterMut<'a, (K, V)>);

impl<'a, K, V> DoubleEndedIterator for ValuesMut<'a, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(_, v)| v)
    }
}

impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| v)
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

fn index<K>(key: &K) -> usize
where
    K: Copy + Into<usize>,
{
    (*key).into()
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

#[test]
fn test_remove() {
    let mut vec = VecMap::new();

    for n in 0..30usize {
        vec.insert(n, n);
    }

    for n in 0..30usize {
        assert_eq!(vec.remove(&n), Some(n));
    }

    assert_eq!(vec.len(), 0);
}
