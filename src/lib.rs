#![forbid(unsafe_code)]
// Copyright (c) 2016 multimap developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

//! A map implementation which allows storing multiple values per key.
//!
//! The interface is roughly based on std::collections::HashMap, but is changed
//! and extended to accomodate the multi-value use case. In fact, MultiMap is
//! implemented mostly as a thin wrapper around std::collections::HashMap and
//! stores its values as a std::Vec per key.
//!
//! Values are guaranteed to be in insertion order as long as not manually
//! changed. Keys are not ordered. Multiple idential key-value-pairs can exist
//! in the MultiMap. A key can exist in the MultiMap with no associated value.
//!
//! # Examples
//!
//! ```
//! use multimap::MultiMap;
//!
//! // create a new MultiMap. An explicit type signature can be omitted because of the
//! // type inference.
//! let mut queries = MultiMap::new();
//!
//! // insert some queries.
//! queries.insert("urls", "http://rust-lang.org");
//! queries.insert("urls", "http://mozilla.org");
//! queries.insert("urls", "http://wikipedia.org");
//! queries.insert("id", "42");
//! queries.insert("name", "roger");
//!
//! // check if there's any urls.
//! println!("Are there any urls in the multimap? {:?}.",
//!     if queries.contains_key("urls") {"Yes"} else {"No"} );
//!
//! // get the first item in a key's vector.
//! assert_eq!(queries.get("urls"), Some(&"http://rust-lang.org"));
//!
//! // get all the urls.
//! assert_eq!(queries.get_slice("urls"),
//!     Some(&vec!["http://rust-lang.org", "http://mozilla.org", "http://wikipedia.org"][..]));
//!
//! // iterate over all keys and the first value in the key's vector.
//! for (key, value) in queries.iter() {
//!     println!("key: {:?}, val: {:?}", key, value);
//! }
//!
//! // iterate over all keys and the key's vector.
//! for (key, values) in queries.iter_all() {
//!     println!("key: {:?}, values: {:?}", key, values);
//! }
//!
//! // the different methods for getting value(s) from the multimap.
//! let mut map = MultiMap::new();
//!
//! map.insert("key1", 42);
//! map.insert("key1", 1337);
//!
//! assert_eq!(map["key1"], 42);
//! assert_eq!(map.get("key1"), Some(&42));
//! assert_eq!(map.get_slice("key1"), Some(&vec![42, 1337][..]));
//! ```

extern crate smallvec;

use std::borrow::Borrow;
use std::collections::hash_map::RandomState;
use std::collections::HashMap;
use std::fmt::{self, Debug};
use std::hash::{BuildHasher, Hash};
use std::iter::{FromIterator, IntoIterator, Iterator};
use std::ops::Index;

use smallvec::{smallvec, SmallVec};
pub use std::collections::hash_map::Iter as IterAll;
pub use std::collections::hash_map::IterMut as IterAllMut;

pub use entry::{Entry, OccupiedEntry, VacantEntry};

mod entry;

/*
#[cfg(feature = "serde_impl")]
pub mod serde;
 */

#[derive(Clone)]
pub struct MultiMap<K, V, S = RandomState, const N: usize = 1> {
    inner: HashMap<K, smallvec::SmallVec<[V; N]>, S>,
}

pub trait MultiMapValue {
    type Item;
    fn as_slice(&mut self) -> &mut [Self::Item];
    fn push(&mut self, value: Self::Item);
    fn pop(&mut self) -> Option<Self::Item>;
}

impl<V, const N: usize> MultiMapValue for &mut smallvec::SmallVec<[V; N]> {
    type Item = V;

    fn as_slice(&mut self) -> &mut [Self::Item] {
        self.as_mut_slice()
    }

    fn push(&mut self, value: Self::Item) {
        smallvec::SmallVec::push(self, value)
    }

    fn pop(&mut self) -> Option<Self::Item> {
        smallvec::SmallVec::pop(self)
    }
}

impl<K, V> MultiMap<K, V>
where
    K: Eq + Hash,
{
    /// Creates an empty MultiMap
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map: MultiMap<&str, isize> = MultiMap::new();
    /// ```
    pub fn new() -> MultiMap<K, V> {
        MultiMap {
            inner: HashMap::new(),
        }
    }

    /// Creates an empty multimap with the given initial capacity.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map: MultiMap<&str, isize> = MultiMap::with_capacity(20);
    /// ```
    pub fn with_capacity(capacity: usize) -> MultiMap<K, V> {
        MultiMap {
            inner: HashMap::with_capacity(capacity),
        }
    }
}

impl<K, V, S> MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    /// Creates an empty MultiMap which will use the given hash builder to hash keys.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map: MultiMap<&str, isize> = MultiMap::with_hasher(s);
    /// ```
    pub fn with_hasher(hash_builder: S) -> MultiMap<K, V, S> {
        MultiMap {
            inner: HashMap::with_hasher(hash_builder),
        }
    }

    /// Creates an empty MultiMap with the given intial capacity and hash builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    /// use std::collections::hash_map::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map: MultiMap<&str, isize> = MultiMap::with_capacity_and_hasher(20, s);
    /// ```
    pub fn with_capacity_and_hasher(capacity: usize, hash_builder: S) -> MultiMap<K, V, S> {
        MultiMap {
            inner: HashMap::with_capacity_and_hasher(capacity, hash_builder),
        }
    }

    /// Inserts a key-value pair into the multimap. If the key does exist in
    /// the map then the value is pushed to that key's vector. If the key doesn't
    /// exist in the map a new vector with the given value is inserted.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert("key", 42);
    /// ```
    pub fn insert(&mut self, k: K, v: V) {
        match self.inner.entry(k) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().push(v);
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(smallvec![v]);
            }
        }
    }

    /// Inserts multiple key-value pairs into the multimap. If the key does exist in
    /// the map then the values are extended into that key's vector. If the key
    /// doesn't exist in the map a new vector collected from the given values is inserted.
    ///
    /// This may be more efficient than inserting values independently.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::<&str, &usize>::new();
    /// map.insert_many("key", &[42, 43]);
    /// ```
    pub fn insert_many<I: IntoIterator<Item = V>>(&mut self, k: K, v: I) {
        match self.inner.entry(k) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().extend(v);
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(v.into_iter().collect::<_>());
            }
        }
    }

    /// Inserts multiple key-value pairs into the multimap. If the key does exist in
    /// the map then the values are extended into that key's vector. If the key
    /// doesn't exist in the map a new vector collected from the given values is inserted.
    ///
    /// This may be more efficient than inserting values independently.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::<&str, usize>::new();
    /// map.insert_many_from_slice("key", &[42, 43]);
    /// ```
    pub fn insert_many_from_slice(&mut self, k: K, v: &[V])
    where
        V: Copy,
    {
        match self.inner.entry(k) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().extend_from_slice(v);
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(SmallVec::from_slice(v));
            }
        }
    }

    /// Returns true if the map contains a value for the specified key.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// assert_eq!(map.contains_key(&1), true);
    /// assert_eq!(map.contains_key(&2), false);
    /// ```
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.contains_key(k)
    }

    /// Returns the number of elements in the map.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(2, 1337);
    /// assert_eq!(map.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Removes a key from the map, returning the vector of values at
    /// the key if the key was previously in the map.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(1, 1337);
    /// assert_eq!(map.remove(&1).map(|i| i.collect::<_>()), Some(vec![42, 1337]));
    /// assert!(map.remove(&1).is_none());
    /// ```
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<impl Iterator<Item = V>>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.remove(k).map(|i| i.into_iter())
    }

    /// Returns a reference to the first item in the vector corresponding to
    /// the key.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(1, 1337);
    /// assert_eq!(map.get(&1), Some(&42));
    /// ```
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.get(k)?.get(0)
    }

    /// Returns a mutable reference to the first item in the vector corresponding to
    /// the key.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(1, 1337);
    /// if let Some(v) = map.get_mut(&1) {
    ///     *v = 99;
    /// }
    /// assert_eq!(map[&1], 99);
    /// ```
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.get_mut(k)?.get_mut(0)
    }

    /// Returns a reference to the vector corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(1, 1337);
    /// assert_eq!(map.get_slice(&1), Some(&[42, 1337][..]));
    /// ```
    pub fn get_slice<Q: ?Sized>(&self, k: &Q) -> Option<&[V]>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.get(k).map(|i| i.as_slice())
    }

    /// Returns a mutable reference to the vector corresponding to the key.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::{MultiMap, MultiMapValue};
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(1, 1337);
    /// if let Some(mut v) = map.get_all_mut(&1) {
    ///     v.as_slice()[0] = 1991;
    ///     v.as_slice()[1] = 2332;
    ///     v.push(111)
    /// }
    /// assert_eq!(map.get_slice(&1), Some(&vec![1991, 2332, 111][..]));
    /// ```
    pub fn get_slice_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut [V]>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.get_mut(k).map(|i| i.as_mut_slice())
    }

    pub fn get_all_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<impl MultiMapValue<Item = V> + '_>
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.inner.get_mut(k)
    }

    /// Returns true if the key is multi-valued.
    ///
    /// The key may be any borrowed form of the map's key type, but Hash and Eq
    /// on the borrowed form must match those for the key type.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1, 42);
    /// map.insert(1, 1337);
    /// map.insert(2, 2332);
    ///
    /// assert_eq!(map.is_vec(&1), true);   // key is multi-valued
    /// assert_eq!(map.is_vec(&2), false);  // key is single-valued
    /// assert_eq!(map.is_vec(&3), false);  // key not in map
    /// ```
    pub fn is_vec<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.get_slice(k) {
            Some(val) => val.len() > 1,
            None => false,
        }
    }

    /// Returns the number of elements the map can hold without reallocating.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let map: MultiMap<usize, usize> = MultiMap::new();
    /// assert!(map.capacity() >= 0);
    /// ```
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Returns true if the map contains no elements.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// assert!(map.is_empty());
    /// map.insert(1,42);
    /// assert!(!map.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clears the map, removing all key-value pairs.
    /// Keeps the allocated memory for reuse.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1,42);
    /// map.clear();
    /// assert!(map.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.inner.clear();
    }

    /// An iterator visiting all keys in arbitrary order.
    /// Iterator element type is &'a K.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1,42);
    /// map.insert(1,1337);
    /// map.insert(2,1337);
    /// map.insert(4,1991);
    ///
    /// let mut keys: Vec<_> = map.keys().collect();
    /// keys.sort();
    /// assert_eq!(keys, [&1, &2, &4]);
    /// ```
    pub fn keys(&'_ self) -> impl Iterator<Item = &K> {
        self.inner.keys()
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The iterator returns
    /// a reference to the key and the first element in the corresponding key's vector.
    /// Iterator element type is (&'a K, &'a V).
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1,42);
    /// map.insert(1,1337);
    /// map.insert(3,2332);
    /// map.insert(4,1991);
    ///
    /// let mut pairs: Vec<_> = map.iter().collect();
    /// pairs.sort_by_key(|p| p.0);
    /// assert_eq!(pairs, [(&1, &42), (&3, &2332), (&4, &1991)]);
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.inner
            .iter()
            .filter_map(|(k, v)| v.first().map(|f| (k, f)))
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The iterator returns
    /// a reference to the key and a mutable reference to the first element in the
    /// corresponding key's vector. Iterator element type is (&'a K, &'a mut V).
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1,42);
    /// map.insert(1,1337);
    /// map.insert(3,2332);
    /// map.insert(4,1991);
    ///
    /// for (_, value) in map.iter_mut() {
    ///     *value *= *value;
    /// }
    ///
    /// let mut pairs: Vec<_> = map.iter_mut().collect();
    /// pairs.sort_by_key(|p| p.0);
    /// assert_eq!(pairs, [(&1, &mut 1764), (&3, &mut 5438224), (&4, &mut 3964081)]);
    /// ```
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.inner
            .iter_mut()
            .filter_map(|(k, v)| v.first_mut().map(|f| (k, f)))
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The iterator returns
    /// a reference to the key and the corresponding key's vector.
    /// Iterator element type is (&'a K, &'a V).
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1,42);
    /// map.insert(1,1337);
    /// map.insert(3,2332);
    /// map.insert(4,1991);
    ///
    /// let mut pairs: Vec<_> = map.iter_all().collect();
    /// pairs.sort_by_key(|p| p.0);
    /// assert_eq!(pairs, [(&1, &vec![42, 1337][..]), (&3, &vec![2332][..]), (&4, &vec![1991][..])]);
    /// ```
    pub fn iter_all(&self) -> impl Iterator<Item = (&K, &[V])> {
        self.inner.iter().map(|(k, v)| (k, v.as_slice()))
    }

    /// An iterator visiting all key-value pairs in arbitrary order. The iterator returns
    /// a reference to the key and the corresponding key's vector.
    /// Iterator element type is (&'a K, &'a V).
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut map = MultiMap::new();
    /// map.insert(1,42);
    /// map.insert(1,1337);
    /// map.insert(3,2332);
    /// map.insert(4,1991);
    ///
    /// for (key, values) in map.iter_all_mut() {
    ///     for value in values.iter_mut() {
    ///         *value = 99;
    ///     }
    /// }
    ///
    /// let mut pairs: Vec<_> = map.iter_all_mut().collect();
    /// pairs.sort_by_key(|p| p.0);
    /// assert_eq!(pairs, [(&1, &mut vec![99, 99][..]), (&3, &mut vec![99][..]), (&4, &mut vec![99][..])]);
    /// ```
    pub fn iter_all_mut(&mut self) -> impl Iterator<Item = (&K, &mut [V])> {
        self.inner.iter_mut().map(|(k, v)| (k, v.as_mut_slice()))
    }

    /*

    /// Gets the specified key's corresponding entry in the map for in-place manipulation.
    /// It's possible to both manipulate the vector and the 'value' (the first value in the
    /// vector).
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut m = MultiMap::new();
    /// m.insert(1, 42);
    ///
    /// {
    ///     let mut v = m.entry(1).or_insert(43);
    ///     assert_eq!(v, &42);
    ///     *v = 44;
    /// }
    /// assert_eq!(m.entry(2).or_insert(666), &666);
    ///
    /// {
    ///     let mut v = m.entry(1).or_insert_vec(vec![43]);
    ///     assert_eq!(v, &vec![44]);
    ///     v.push(50);
    /// }
    /// assert_eq!(m.entry(2).or_insert_vec(vec![667]), &vec![666]);
    ///
    /// assert_eq!(m.get_slice(&1), Some(&vec![44, 50][..]));
    /// ```
    pub fn entry(&mut self, k: K) -> Entry<K, V> {
        use std::collections::hash_map::Entry as HashMapEntry;
        match self.inner.entry(k) {
            HashMapEntry::Occupied(entry) => Entry::Occupied(OccupiedEntry { inner: entry }),
            HashMapEntry::Vacant(entry) => Entry::Vacant(VacantEntry { inner: entry }),
        }
    }

    */

    /// Retains only the elements specified by the predicate.
    ///
    /// In other words, remove all pairs `(k, v)` such that `f(&k,&mut v)` returns `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// use multimap::MultiMap;
    ///
    /// let mut m = MultiMap::new();
    /// m.insert(1, 42);
    /// m.insert(1, 99);
    /// m.insert(2, 42);
    /// m.retain(|&k, &v| { k == 1 && v == 42 });
    /// assert_eq!(1, m.len());
    /// assert_eq!(Some(&42), m.get(&1));
    /// ```
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&K, &V) -> bool,
    {
        for (k, v) in self.inner.iter_mut() {
            v.retain(|iv| f(k, iv))
        }

        self.inner.retain(|_, v| !v.is_empty());
    }
}

impl<'a, K, V, S, Q: ?Sized> Index<&'a Q> for MultiMap<K, V, S>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash,
    S: BuildHasher,
{
    type Output = V;

    fn index(&self, index: &Q) -> &V {
        self.inner
            .get(index)
            .map(|v| &v[0])
            .expect("no entry found for key")
    }
}

impl<K, V, S> Debug for MultiMap<K, V, S>
where
    K: Eq + Hash + Debug,
    V: Debug,
    S: BuildHasher,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_map().entries(self.iter_all()).finish()
    }
}

impl<K, V, S> PartialEq for MultiMap<K, V, S>
where
    K: Eq + Hash,
    V: PartialEq,
    S: BuildHasher,
{
    fn eq(&self, other: &MultiMap<K, V, S>) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter_all()
            .all(|(key, value)| other.get_slice(key).map_or(false, |v| *value == *v))
    }
}

impl<K, V, S> Eq for MultiMap<K, V, S>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
{
}

impl<K, V, S> Default for MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    fn default() -> MultiMap<K, V, S> {
        MultiMap {
            inner: Default::default(),
        }
    }
}

impl<K, V, S> FromIterator<(K, V)> for MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iterable: T) -> MultiMap<K, V, S> {
        let iter = iterable.into_iter();
        let hint = iter.size_hint().0;

        let mut multimap = MultiMap::with_capacity_and_hasher(hint, S::default());
        for (k, v) in iter {
            multimap.insert(k, v);
        }

        multimap
    }
}

/*
impl<'a, K, V, S> IntoIterator for &'a MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Item = (&'a K, &'a [V]);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_all()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Item = (&'a K, &'a mut Vec<V>);
    type IntoIter = IterAllMut<'a, K, Vec<V>>;

    fn into_iter(self) -> IterAllMut<'a, K, Vec<V>> {
        self.inner.iter_mut()
    }
}

impl<K, V, S> IntoIterator for MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    type Item = (K, Vec<V>);
    type IntoIter = IntoIter<K, Vec<V>>;

    fn into_iter(self) -> IntoIter<K, Vec<V>> {
        self.inner.into_iter()
    }
}
 */

impl<K, V, S> Extend<(K, V)> for MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }
}

impl<'a, K, V, S> Extend<(&'a K, &'a V)> for MultiMap<K, V, S>
where
    K: Eq + Hash + Copy,
    V: Copy,
    S: BuildHasher,
{
    fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
        self.extend(iter.into_iter().map(|(&key, &value)| (key, value)));
    }
}

impl<K, V, S> Extend<(K, Vec<V>)> for MultiMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn extend<T: IntoIterator<Item = (K, Vec<V>)>>(&mut self, iter: T) {
        for (k, values) in iter {
            match self.inner.entry(k) {
                std::collections::hash_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().extend(values);
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(SmallVec::from(values));
                }
            }
        }
    }
}

impl<'a, K, V, S> Extend<(&'a K, &'a Vec<V>)> for MultiMap<K, V, S>
where
    K: Eq + Hash + Copy,
    V: Copy,
    S: BuildHasher,
{
    fn extend<T: IntoIterator<Item = (&'a K, &'a Vec<V>)>>(&mut self, iter: T) {
        self.extend(
            iter.into_iter()
                .map(|(&key, values)| (key, values.to_owned())),
        );
    }
}

#[derive(Clone)]
pub struct Iter<'a, K: 'a, V: 'a> {
    inner: IterAll<'a, K, Vec<V>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        self.inner.next().map(|(k, v)| (k, &v[0]))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for Iter<'a, K, V> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

pub struct IterMut<'a, K: 'a, V: 'a> {
    inner: IterAllMut<'a, K, Vec<V>>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        self.inner.next().map(|(k, v)| (k, &mut v[0]))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<'a, K, V> ExactSizeIterator for IterMut<'a, K, V> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

#[macro_export]
/// Create a `MultiMap` from a list of key value pairs
///
/// ## Example
///
/// ```
/// # use multimap::MultiMap;
/// #[macro_use] extern crate multimap;
/// # fn main(){
///
/// let map = multimap!(
///     "dog" => "husky",
///     "dog" => "retreaver",
///     "dog" => "shiba inu",
///     "cat" => "cat"
///     );
/// # }
///
/// ```
macro_rules! multimap{
    (@replace_with_unit $_t:tt) => { () };
    (@count $($key:expr),*) => { <[()]>::len(&[$($crate::multimap! { @replace_with_unit $key }),*]) };

    ($($key:expr => $value:expr),* $(,)?)=>{
        {
            let mut map = $crate::MultiMap::with_capacity($crate::multimap! { @count $($key),* });
            $(
                map.insert($key,$value);
             )*
            map
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::iter::FromIterator;

    use super::*;

    #[test]
    fn create() {
        let _: MultiMap<usize, usize> = MultiMap {
            inner: HashMap::new(),
        };
    }

    #[test]
    fn new() {
        let _: MultiMap<usize, usize> = MultiMap::new();
    }

    #[test]
    fn with_capacity() {
        let _: MultiMap<usize, usize> = MultiMap::with_capacity(20);
    }

    #[test]
    fn insert() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 3);
    }

    #[test]
    fn insert_identical() {
        let mut m = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 42);
        assert_eq!(m.get_slice(&1), Some(&vec![42, 42][..]));
    }

    #[test]
    fn insert_many() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert_many(1, vec![3, 4]);
        assert_eq!(Some(&vec![3, 4][..]), m.get_slice(&1));
    }

    #[test]
    fn insert_many_again() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 2);
        m.insert_many(1, vec![3, 4]);
        assert_eq!(Some(&vec![2, 3, 4][..]), m.get_slice(&1));
    }

    #[test]
    fn insert_many_overlap() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert_many(1, vec![2, 3]);
        m.insert_many(1, vec![3, 4]);
        assert_eq!(Some(&vec![2, 3, 3, 4][..]), m.get_slice(&1));
    }

    #[test]
    fn insert_many_from_slice() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert_many_from_slice(1, &[3, 4]);
        assert_eq!(Some(&vec![3, 4][..]), m.get_slice(&1));
    }

    #[test]
    fn insert_many_from_slice_again() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 2);
        m.insert_many_from_slice(1, &[3, 4]);
        assert_eq!(Some(&vec![2, 3, 4][..]), m.get_slice(&1));
    }

    #[test]
    fn insert_existing() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 3);
        m.insert(1, 4);
        assert_eq!(Some(&vec![3, 4][..]), m.get_slice(&1));
    }

    #[test]
    #[should_panic]
    fn index_no_entry() {
        let m: MultiMap<usize, usize> = MultiMap::new();
        let _ = &m[&1];
    }

    #[test]
    fn index() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        let values = m[&1];
        assert_eq!(values, 42);
    }

    #[test]
    fn contains_key_true() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        assert!(m.contains_key(&1));
    }

    #[test]
    fn contains_key_false() {
        let m: MultiMap<usize, usize> = MultiMap::new();
        assert!(!m.contains_key(&1));
    }

    #[test]
    fn len() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(2, 1337);
        m.insert(3, 99);
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn remove_not_present() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        let v = m.remove(&1);
        assert!(v.is_none());
    }

    #[test]
    fn remove_present() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        let v = m.remove(&1);
        assert_eq!(Some(vec![42]), v.map(|i| i.collect::<_>()));
    }

    #[test]
    fn get_not_present() {
        let m: MultiMap<usize, usize> = MultiMap::new();
        assert_eq!(m.get(&1), None);
    }

    #[test]
    fn get_present() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        assert_eq!(m.get(&1), Some(&42));
    }

    #[test]
    fn get_empty() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.remove(&1);
        assert_eq!(m.get(&1), None);
    }

    #[test]
    fn get_slice_not_present() {
        let m: MultiMap<usize, usize> = MultiMap::new();
        assert_eq!(m.get_slice(&1), None);
    }

    #[test]
    fn get_slice_present() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 1337);
        assert_eq!(Some(&vec![42, 1337][..]), m.get_slice(&1));
    }

    #[test]
    fn capacity() {
        let m: MultiMap<usize, usize> = MultiMap::with_capacity(20);
        assert!(m.capacity() >= 20);
    }

    #[test]
    fn is_empty_true() {
        let m: MultiMap<usize, usize> = MultiMap::new();
        assert!(m.is_empty());
    }

    #[test]
    fn is_empty_false() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        assert!(!m.is_empty());
    }

    #[test]
    fn clear() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.clear();
        assert!(m.is_empty());
    }

    #[test]
    fn get_mut() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        if let Some(v) = m.get_mut(&1) {
            *v = 1337;
        }
        assert_eq!(m[&1], 1337)
    }

    #[test]
    fn get_all_mut() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 1337);
        if let Some(mut v) = m.get_all_mut(&1) {
            v.push(555);
            (*v.as_slice())[0] = 5;
            (*v.as_slice())[1] = 10;
            (*v.as_slice())[2] = 55;
        }
        assert_eq!(Some(&vec![5, 10, 55][..]), m.get_slice(&1));
    }

    #[test]
    fn get_slice_mut() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 1337);
        if let Some(v) = m.get_slice_mut(&1) {
            (*v)[0] = 5;
            (*v)[1] = 10;
        }
        assert_eq!(Some(&vec![5, 10][..]), m.get_slice(&1));
    }

    #[test]
    fn get_mut_empty() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.get_all_mut(&1).and_then(|mut v| v.pop());
        assert_eq!(m.get_mut(&1), None);
    }

    #[test]
    fn keys() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(2, 42);
        m.insert(4, 42);
        m.insert(8, 42);

        let keys: Vec<_> = m.keys().cloned().collect();
        assert_eq!(keys.len(), 4);
        assert!(keys.contains(&1));
        assert!(keys.contains(&2));
        assert!(keys.contains(&4));
        assert!(keys.contains(&8));
    }

    #[test]
    fn iter() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 42);
        m.insert(4, 42);
        m.insert(8, 42);

        let mut iter = m.iter();

        for _ in iter.by_ref().take(2) {}

        assert_eq!(iter.count(), 1);
    }

    #[test]
    fn intoiterator_for_reference_type() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 43);
        m.insert(4, 42);
        m.insert(8, 42);

        let keys = vec![1, 4, 8];

        for (key, value) in m.iter_all() {
            assert!(keys.contains(key));

            if key == &1 {
                assert_eq!(value, &vec![42, 43][..]);
            } else {
                assert_eq!(value, &vec![42][..]);
            }
        }
    }

    /*
    #[test]
    fn intoiterator_for_mutable_reference_type() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 43);
        m.insert(4, 42);
        m.insert(8, 42);

        let keys = vec![1, 4, 8];

        for (key, value) in &mut m {
            assert!(keys.contains(key));

            if key == &1 {
                assert_eq!(value, &vec![42, 43]);
                value.push(666);
            } else {
                assert_eq!(value, &vec![42]);
            }
        }

        assert_eq!(m.get_slice(&1), Some(&vec![42, 43, 666][..]));
    }
     */

    #[test]
    fn intoiterator_consuming() {
        let mut m: MultiMap<usize, usize> = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 43);
        m.insert(4, 42);
        m.insert(8, 42);

        let keys = vec![1, 4, 8];

        for (key, value) in m.iter_all() {
            assert!(keys.contains(&key));

            if key == &1 {
                assert_eq!(value, &vec![42, 43][..]);
            } else {
                assert_eq!(value, &vec![42][..]);
            }
        }
    }

    #[test]
    fn test_fmt_debug() {
        let mut map = MultiMap::new();
        let empty: MultiMap<i32, i32> = MultiMap::new();

        map.insert(1, 2);
        map.insert(1, 5);
        map.insert(1, -1);
        map.insert(3, 4);

        let map_str = format!("{:?}", map);

        assert!(map_str == "{1: [2, 5, -1], 3: [4]}" || map_str == "{3: [4], 1: [2, 5, -1]}");
        assert_eq!(format!("{:?}", empty), "{}");
    }

    #[test]
    fn test_eq() {
        let mut m1 = MultiMap::new();
        m1.insert(1, 2);
        m1.insert(2, 3);
        m1.insert(3, 4);
        let mut m2 = MultiMap::new();
        m2.insert(1, 2);
        m2.insert(2, 3);
        assert_ne!(m1, m2);
        m2.insert(3, 4);
        assert_eq!(m1, m2);
        m2.insert(3, 4);
        assert_ne!(m1, m2);
        m1.insert(3, 4);
        assert_eq!(m1, m2);
    }

    #[test]
    fn test_eq_empty_key() {
        let mut m1 = MultiMap::new();
        m1.insert(1, 2);
        m1.insert(2, 3);
        let mut m2 = MultiMap::new();
        m2.insert(1, 2);
        m2.insert_many(2, []);
        assert_ne!(m1, m2);
        m2.insert_many(2, [3]);
        assert_eq!(m1, m2);
    }

    #[test]
    fn test_default() {
        let _: MultiMap<u8, u8> = Default::default();
    }

    #[test]
    fn test_from_iterator() {
        let vals: Vec<(&str, i64)> = vec![("foo", 123), ("bar", 456), ("foo", 789)];
        let multimap: MultiMap<&str, i64> = MultiMap::from_iter(vals);

        let foo_vals: &[i64] = multimap.get_slice("foo").unwrap();
        assert!(foo_vals.contains(&123));
        assert!(foo_vals.contains(&789));

        let bar_vals: &[i64] = multimap.get_slice("bar").unwrap();
        assert!(bar_vals.contains(&456));
    }

    #[test]
    fn test_extend_consuming_hashmap() {
        let mut a = MultiMap::new();
        a.insert(1, 42);

        let mut b = HashMap::new();
        b.insert(1, 43);
        b.insert(2, 666);

        a.extend(b);

        assert_eq!(a.len(), 2);
        assert_eq!(a.get_slice(&1), Some(&vec![42, 43][..]));
    }

    #[test]
    fn test_extend_ref_hashmap() {
        let mut a = MultiMap::new();
        a.insert(1, 42);

        let mut b = HashMap::new();
        b.insert(1, 43);
        b.insert(2, 666);

        a.extend(&b);

        assert_eq!(a.len(), 2);
        assert_eq!(a.get_slice(&1), Some(&vec![42, 43][..]));
        assert_eq!(b.len(), 2);
        assert_eq!(b[&1], 43);
    }

    /*
    #[test]
    fn test_extend_consuming_multimap() {
        let mut a = MultiMap::new();
        a.insert(1, 42);

        let mut b = MultiMap::new();
        b.insert(1, 43);
        b.insert(1, 44);
        b.insert(2, 666);

        a.extend(b);

        assert_eq!(a.len(), 2);
        assert_eq!(a.get_slice(&1), Some(&vec![42, 43, 44][..]));
    }

    #[test]
    fn test_extend_ref_multimap() {
        let mut a = MultiMap::new();
        a.insert(1, 42);

        let mut b = MultiMap::new();
        b.insert(1, 43);
        b.insert(1, 44);
        b.insert(2, 666);

        a.extend(&b);

        assert_eq!(a.len(), 2);
        assert_eq!(a.get_slice(&1), Some(&vec![42, 43, 44][..]));
        assert_eq!(b.len(), 2);
        assert_eq!(b.get_slice(&1), Some(&vec![43, 44][..]));
    }

    #[test]
    fn test_entry() {
        let mut m = MultiMap::new();
        m.insert(1, 42);

        {
            let v = m.entry(1).or_insert(43);
            assert_eq!(v, &42);
            *v = 44;
        }
        assert_eq!(m.entry(2).or_insert(666), &666);

        assert_eq!(m[&1], 44);
        assert_eq!(m[&2], 666);
    }

    #[test]
    fn test_entry_vec() {
        let mut m = MultiMap::new();
        m.insert(1, 42);

        {
            let v = m.entry(1).or_insert_vec(vec![43]);
            assert_eq!(v, &vec![42]);
            *v.first_mut().unwrap() = 44;
        }
        assert_eq!(m.entry(2).or_insert_vec(vec![666]), &vec![666]);

        assert_eq!(m[&1], 44);
        assert_eq!(m[&2], 666);
    }
     */

    #[test]
    fn test_is_vec() {
        let mut m = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 1337);
        m.insert(2, 2332);

        assert!(m.is_vec(&1));
        assert!(!m.is_vec(&2));
        assert!(!m.is_vec(&3));
    }

    #[test]
    fn test_macro() {
        let mut manual_map = MultiMap::new();
        manual_map.insert("key1", 42);
        assert_eq!(manual_map, multimap!("key1" => 42));

        manual_map.insert("key1", 1337);
        manual_map.insert("key2", 2332);
        let macro_map = multimap! {
            "key1" =>    42,
            "key1" =>  1337,
            "key2" =>  2332
        };
        assert_eq!(manual_map, macro_map);
    }

    #[test]
    fn retain_removes_element() {
        let mut m = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 99);
        m.retain(|&k, &v| k == 1 && v == 42);
        assert_eq!(1, m.len());
        assert_eq!(Some(&42), m.get(&1));
    }

    #[test]
    fn retain_also_removes_empty_vector() {
        let mut m = MultiMap::new();
        m.insert(1, 42);
        m.insert(1, 99);
        m.insert(2, 42);
        m.retain(|&k, &v| k == 1 && v == 42);
        assert_eq!(1, m.len());
        assert_eq!(Some(&42), m.get(&1));
    }
}
