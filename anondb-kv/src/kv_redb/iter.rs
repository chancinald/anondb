use std::sync::Arc;

use anyhow::Result;

pub struct MaybeEmptyIter<I: Iterator> {
    inner_iter: Option<I>,
}

impl<I: Iterator> Default for MaybeEmptyIter<I> {
    fn default() -> Self {
        Self { inner_iter: None }
    }
}

impl<I: Iterator> From<I> for MaybeEmptyIter<I> {
    fn from(value: I) -> Self {
        Self {
            inner_iter: Some(value),
        }
    }
}

impl<I: Iterator> Iterator for MaybeEmptyIter<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = self.inner_iter.as_mut() {
            iter.next()
        } else {
            None
        }
    }
}

/// A mapping utility for fallible iterators.
pub struct RedbReadIter<T, I: Iterator, D> {
    pub data: Arc<D>,
    pub inner_iter: I,
    pub map_fn: fn(Arc<D>, I::Item) -> T,
}

impl<T, I: Iterator, D> Iterator for RedbReadIter<T, I, D> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let data = self.data.clone();
        self.inner_iter.next().map(|v| (self.map_fn)(data, v))
    }
}

/// Take an iterator that returns Result<Iterator<Result<T>>> and flatten it.
pub struct FlatMapFallible<T, I: Iterator<Item = Result<V>>, V: Iterator<Item = Result<T>>> {
    inner_iter: I,
    to_be_flattened: Option<V>,
}

impl<T, I: Iterator<Item = Result<V>>, V: Iterator<Item = Result<T>>> Iterator
    for FlatMapFallible<T, I, V>
{
    type Item = Result<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(t) = self.to_be_flattened.as_mut() {
            let n = t.next();
            if n.is_none() {
                self.to_be_flattened = None;
                self.next()
            } else {
                n
            }
        } else {
            if let Some(n) = self.inner_iter.next() {
                match n {
                    Ok(v) => {
                        self.to_be_flattened = Some(v);
                        self.next()
                    }
                    // this will loop infinitely?
                    Err(e) => Some(Err(e)),
                }
            } else {
                None
            }
        }
    }
}

impl<T, I: Iterator<Item = Result<V>>, V: Iterator<Item = Result<T>>> From<I>
    for FlatMapFallible<T, I, V>
{
    fn from(value: I) -> Self {
        Self {
            inner_iter: value,
            to_be_flattened: None,
        }
    }
}
