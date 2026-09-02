#[cfg(feature = "rayon")]
mod rayon_impl;

#[cfg(feature = "rayon")]
pub use rayon_impl::*;

#[cfg(feature = "serde")]
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{MapAccess, Visitor},
};
#[cfg(feature = "serde")]
use std::marker::PhantomData;

use std::{
    fmt::{self, Debug},
    iter::{FromIterator, FusedIterator},
    mem::replace,
    num::NonZeroU32,
};

pub struct VecMap<K, V> {
    keys: Vec<Option<NonZeroU32>>,
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
        K: Clone + Into<u32>,
    {
        self.keys.get(index(key)).is_some_and(Option::is_some)
    }

    #[must_use]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V>
    where
        K: Clone + Into<u32>,
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
        K: Clone + Into<u32>,
    {
        let row_index = row_index(self.keys.get(index(key)).and_then(|index| *index)?);

        unsafe { Some(&self.rows.get_unchecked(row_index).1) }
    }

    #[inline]
    #[must_use]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V>
    where
        K: Clone + Into<u32>,
    {
        let row_index = row_index(self.keys.get(index(key)).and_then(|index| *index)?);

        unsafe { Some(&mut self.rows.get_unchecked_mut(row_index).1) }
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V>
    where
        K: Clone + Into<u32>,
    {
        let key_index = key.clone().into() as usize;

        let index = match self.keys.get_mut(key_index) {
            Some(index) => index,
            None => {
                self.keys
                    .extend((self.keys.len()..=key_index).map(|_| None));

                unsafe { self.keys.get_unchecked_mut(key_index) }
            }
        };

        match index {
            &mut Some(index) => Some(replace(
                &mut unsafe { self.rows.get_unchecked_mut(row_index(index)) }.1,
                value,
            )),
            None => {
                *index = Some(stored_index(self.rows.len()));
                self.rows.push((key, value));
                None
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter(self.rows.iter())
    }

    #[inline]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut(self.rows.iter_mut())
    }

    #[inline]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys(self.rows.iter())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn remove(&mut self, key: &K) -> Option<V>
    where
        K: Clone + Into<u32>,
    {
        if let Some(row_index) = self
            .keys
            .get_mut(index(key))
            .and_then(Option::take)
            .map(row_index)
        {
            if self.rows.len() - 1 != row_index
                && let Some(k) = self.rows.last().map(|t| index(&t.0))
            {
                *self.keys.get_mut(k).expect("key") = Some(stored_index(row_index));
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
    /// map.insert(1u32, 10);
    /// map.insert(2u32, 11);
    ///
    /// map.retain(|_k, v| v > &10);
    ///
    /// assert_eq!(map.len(), 1);
    /// assert_eq!(map.into_iter().collect::<Vec<_>>(), vec![(2, 11)]);
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &V) -> bool,
        K: Clone,
    {
        let rows_len = self.rows.len();
        let mut old_index = 0;
        let mut next_index = 0;
        let mut index_map: Option<Vec<Option<NonZeroU32>>> = None;

        self.rows.retain(|t| {
            let retain = f(&t.0, &t.1);

            if retain {
                if let Some(index_map) = &mut index_map {
                    index_map.push(Some(stored_index(next_index)));
                }
                next_index += 1;
            } else {
                match &mut index_map {
                    Some(index_map) => index_map.push(None),
                    None => {
                        let mut new_index_map = Vec::with_capacity(rows_len);
                        new_index_map.extend((0..old_index).map(|index| Some(stored_index(index))));
                        new_index_map.push(None);
                        index_map = Some(new_index_map);
                    }
                }
            }

            old_index += 1;
            retain
        });

        if let Some(index_map) = index_map {
            for key in &mut self.keys {
                if let Some(index) = *key {
                    *key = index_map[row_index(index)];
                }
            }
        }
    }

    pub fn shrink_to_fit(&mut self) {
        if let Some(index) = self.keys.iter().rposition(Option::is_some) {
            self.keys.truncate(index + 1);
        } else {
            self.keys.clear();
        }

        self.keys.shrink_to_fit();
        self.rows.shrink_to_fit();
    }

    #[inline]
    pub fn values(&self) -> Values<'_, K, V> {
        Values(self.rows.iter())
    }

    #[inline]
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
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
    K: Clone + Into<u32>,
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
    K: Clone + Into<u32>,
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

impl<'a, K, V> IntoIterator for &'a VecMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V> IntoIterator for &'a mut VecMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<K, V> Eq for VecMap<K, V>
where
    K: Clone + Eq + Into<u32>,
    V: Eq,
{
}

impl<K, V> PartialEq for VecMap<K, V>
where
    K: Clone + Into<u32> + PartialEq,
    V: PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) || self.rows == other.rows {
            return true;
        }

        self.len() == other.len()
            && self.rows.iter().all(|(key, value)| {
                other
                    .keys
                    .get(index(key))
                    .and_then(|index| *index)
                    .map(row_index)
                    .is_some_and(|index| {
                        let (other_key, other_value) = unsafe { other.rows.get_unchecked(index) };
                        key == other_key && value == other_value
                    })
            })
    }
}

#[cfg(feature = "serde")]
impl<'de, K, V> Deserialize<'de> for VecMap<K, V>
where
    K: Clone + Deserialize<'de> + Into<u32>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(Visit(PhantomData))
    }
}

#[cfg(feature = "serde")]
impl<K: Serialize, V: Serialize> Serialize for VecMap<K, V> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_map(self.rows.iter().map(|t| (&t.0, &t.1)))
    }
}

pub enum Entry<'a, K: 'a, V: 'a> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K, V> Entry<'a, K, V> {
    pub fn or_insert(self, default: V) -> &'a mut V
    where
        K: Clone + Into<u32>,
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
    /// let mut map: VecMap<u32, String> = VecMap::new();
    /// let s = "hoho".to_string();
    ///
    /// map.entry(2).or_insert_with(|| s);
    ///
    /// assert_eq!(map.get(&2).unwrap().clone(), "hoho".to_string());
    /// ```
    pub fn or_insert_with<F>(self, default: F) -> &'a mut V
    where
        F: FnOnce() -> V,
        K: Clone + Into<u32>,
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
        K: Clone + Into<u32>,
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
        K: Clone + Into<u32>,
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

impl<K, V> ExactSizeIterator for IntoIter<K, V> {}

impl<K, V> FusedIterator for IntoIter<K, V> {}

pub struct Iter<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<K, V> Clone for Iter<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K, V> DoubleEndedIterator for Iter<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|t| (&t.0, &t.1))
    }
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|t| (&t.0, &t.1))
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

impl<K, V> ExactSizeIterator for Iter<'_, K, V> {}

impl<K, V> FusedIterator for Iter<'_, K, V> {}

pub struct IterMut<'a, K, V>(std::slice::IterMut<'a, (K, V)>);

impl<K, V> DoubleEndedIterator for IterMut<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|t| (&t.0, &mut t.1))
    }
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|t| (&t.0, &mut t.1))
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

impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {}

impl<K, V> FusedIterator for IterMut<'_, K, V> {}

pub struct Keys<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<K, V> Clone for Keys<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K, V> DoubleEndedIterator for Keys<'_, K, V> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(|(k, _)| k)
    }
}

impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

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

impl<K, V> ExactSizeIterator for Keys<'_, K, V> {}

impl<K, V> FusedIterator for Keys<'_, K, V> {}

pub struct OccupiedEntry<'a, K, V> {
    key: K,
    vec: &'a mut VecMap<K, V>,
}

impl<'a, K, V> OccupiedEntry<'a, K, V> {
    pub fn get(&self) -> &V
    where
        K: Clone + Into<u32>,
    {
        self.vec.get(&self.key).unwrap()
    }

    pub fn get_mut(&mut self) -> &mut V
    where
        K: Clone + Into<u32>,
    {
        self.vec.get_mut(&self.key).unwrap()
    }

    pub fn insert(&mut self, value: V) -> V
    where
        K: Clone + Into<u32>,
    {
        self.vec.insert(self.key.clone(), value).unwrap()
    }

    pub fn into_mut(self) -> &'a mut V
    where
        K: Clone + Into<u32>,
    {
        self.vec.get_mut(&self.key).unwrap()
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn remove(self) -> V
    where
        K: Clone + Into<u32>,
    {
        self.vec.remove(&self.key).unwrap()
    }

    pub fn remove_entry(self) -> (K, V)
    where
        K: Clone + Into<u32>,
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
        K: Clone + Into<u32>,
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

pub struct Values<'a, K, V>(std::slice::Iter<'a, (K, V)>);

impl<K, V> Clone for Values<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K, V> DoubleEndedIterator for Values<'_, K, V> {
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

impl<K, V> ExactSizeIterator for Values<'_, K, V> {}

impl<K, V> FusedIterator for Values<'_, K, V> {}

pub struct ValuesMut<'a, K, V>(std::slice::IterMut<'a, (K, V)>);

impl<K, V> DoubleEndedIterator for ValuesMut<'_, K, V> {
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

impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {}

impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}

#[cfg(feature = "serde")]
struct Visit<K, V>(PhantomData<(K, V)>);

#[cfg(feature = "serde")]
impl<'de, K, V> Visitor<'de> for Visit<K, V>
where
    K: Clone + Deserialize<'de> + Into<u32>,
    V: Deserialize<'de>,
{
    type Value = VecMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("VecMap")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = VecMap::with_capacity(access.size_hint().unwrap_or(0));

        while let Some((key, value)) = access.next_entry()? {
            map.insert(key, value);
        }

        Ok(map)
    }
}

#[inline]
fn index<K>(key: &K) -> usize
where
    K: Clone + Into<u32>,
{
    (key.clone()).into() as usize
}

#[inline]
fn row_index(index: NonZeroU32) -> usize {
    (index.get() - 1) as usize
}

#[inline]
fn stored_index(index: usize) -> NonZeroU32 {
    NonZeroU32::new(u32::try_from(index + 1).expect("too many rows")).unwrap()
}

#[test]
fn test_insert() {
    let mut vec = VecMap::new();

    for n in (0..30u32).rev() {
        assert!(vec.insert(n, n).is_none());
    }

    assert_eq!(vec.len(), 30);

    for n in (0..30u32).rev() {
        let old = vec.insert(n, 100 - n);

        assert_eq!(n, old.unwrap());
    }

    assert_eq!(vec.len(), 30);
}

#[test]
fn test_key_index_uses_compact_representation() {
    assert_eq!(
        std::mem::size_of::<Option<NonZeroU32>>(),
        std::mem::size_of::<u32>()
    );
}

#[test]
fn test_equality_is_independent_of_row_order() {
    let left = [(1u32, 10), (2, 20), (3, 30)]
        .into_iter()
        .collect::<VecMap<_, _>>();
    let same_order = left.clone();
    let mut right = [(3u32, 30), (1, 10), (2, 20)]
        .into_iter()
        .collect::<VecMap<_, _>>();

    assert!(left == left);
    assert!(left == same_order);
    assert!(left == right);

    assert_eq!(right.remove(&1), Some(10));
    right.insert(1, 10);
    assert!(left == right);

    right.insert(2, 200);
    assert!(left != right);
    right.remove(&3);
    assert!(left != right);
}

#[test]
fn test_remove() {
    let mut vec = VecMap::new();

    assert_eq!(vec.remove(&0), None);

    for n in 0..30u32 {
        vec.insert(n, n);
    }

    assert_eq!(vec.remove(&29), Some(29));
    assert_eq!(vec.remove(&29), None);

    assert_eq!(vec.remove(&0), Some(0));
    assert_eq!(vec.get(&28), Some(&28));
    assert!(vec.contains_key(&28));
    assert_eq!(vec.insert(28, 128), Some(28));
    assert_eq!(vec.get(&28), Some(&128));

    for n in 1..29u32 {
        let expected = if n == 28 { 128 } else { n };
        assert_eq!(vec.remove(&n), Some(expected));
    }

    assert_eq!(vec.remove(&30), None);
    assert_eq!(vec.len(), 0);
}

#[test]
fn test_retain_keeps_all() {
    let mut vec = VecMap::new();

    for n in 0..5u32 {
        vec.insert(n, n * 10);
    }

    let mut visited = Vec::new();
    vec.retain(|k, v| {
        visited.push((*k, *v));
        true
    });

    assert_eq!(visited, vec![(0, 0), (1, 10), (2, 20), (3, 30), (4, 40)]);
    assert_eq!(vec.len(), 5);
    assert_eq!(vec.into_iter().collect::<Vec<_>>(), visited);
}

#[test]
fn test_retain_removes_all() {
    let mut vec = VecMap::new();

    for n in 0..5u32 {
        vec.insert(n, n);
    }

    vec.retain(|_, _| false);

    assert!(vec.is_empty());

    for n in 0..5u32 {
        assert_eq!(vec.get(&n), None);
        assert!(!vec.contains_key(&n));
        assert_eq!(vec.insert(n, n + 10), None);
        assert_eq!(vec.remove(&n), Some(n + 10));
    }
}

#[test]
fn test_retain_updates_indices_and_preserves_order() {
    let mut vec = VecMap::new();

    for (key, value) in [(2u32, 20), (8, 80), (3, 30), (15, 150), (5, 50)] {
        vec.insert(key, value);
    }

    vec.retain(|k, _| *k % 2 == 1);

    assert_eq!(vec.len(), 3);
    assert_eq!(
        vec.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
        vec![(3, 30), (15, 150), (5, 50)]
    );
    assert_eq!(vec.get(&2), None);
    assert_eq!(vec.get(&8), None);
    assert_eq!(vec.get(&3), Some(&30));
    assert_eq!(vec.get(&15), Some(&150));
    assert_eq!(vec.get(&5), Some(&50));
    assert_eq!(vec.insert(15, 151), Some(150));
    assert_eq!(vec.remove(&5), Some(50));
    assert_eq!(vec.remove(&3), Some(30));
    assert_eq!(vec.remove(&15), Some(151));
    assert!(vec.is_empty());
}

#[test]
fn test_shrink_to_fit_preserves_last_key_and_releases_empty_index() {
    let mut vec = VecMap::new();

    vec.insert(2u32, 20);
    vec.insert(100, 1000);
    vec.insert(200, 2000);
    assert_eq!(vec.remove(&200), Some(2000));

    vec.shrink_to_fit();

    assert_eq!(vec.keys.len(), 101);
    assert_eq!(vec.get(&100), Some(&1000));
    assert_eq!(vec.remove(&2), Some(20));
    assert_eq!(vec.remove(&100), Some(1000));

    vec.shrink_to_fit();

    assert!(vec.keys.is_empty());
    assert_eq!(vec.keys.capacity(), 0);
    assert_eq!(vec.insert(1, 10), None);
    assert_eq!(vec.get(&1), Some(&10));
}
