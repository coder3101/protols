use std::{collections::HashSet, ops::Deref};

use async_lsp::lsp_types::Url;

/// A type-safe container that always holds unique and sorted URLs.
pub struct UniqueUris {
    uris: Vec<Url>,
}

impl UniqueUris {
    #[inline]
    pub fn new<T>(source: T) -> Self
    where
        T: IntoUniqueUris,
    {
        source.into_unique_uris()
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Url> {
        self.uris.iter()
    }

    #[inline]
    pub fn into_inner(self) -> Vec<Url> {
        self.uris
    }
}

impl Deref for UniqueUris {
    type Target = [Url];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.uris
    }
}

impl IntoIterator for UniqueUris {
    type Item = Url;
    type IntoIter = std::vec::IntoIter<Url>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.uris.into_iter()
    }
}

impl<'a> IntoIterator for &'a UniqueUris {
    type Item = &'a Url;
    type IntoIter = std::slice::Iter<'a, Url>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.uris.iter()
    }
}

pub trait IntoUniqueUris {
    fn into_unique_uris(self) -> UniqueUris;
}

impl IntoUniqueUris for HashSet<Url> {
    #[inline]
    fn into_unique_uris(self) -> UniqueUris {
        let uris: Vec<Url> = self.into_iter().collect();
        UniqueUris { uris }
    }
}

impl<V> IntoUniqueUris for std::collections::hash_map::IntoKeys<Url, V> {
    #[inline]
    fn into_unique_uris(self) -> UniqueUris {
        UniqueUris {
            uris: self.collect(),
        }
    }
}

impl IntoUniqueUris for Vec<Url> {
    #[inline]
    fn into_unique_uris(self) -> UniqueUris {
        let mut uris = self;
        uris.sort_unstable();
        uris.dedup();
        UniqueUris { uris }
    }
}

impl IntoUniqueUris for Url {
    #[inline]
    fn into_unique_uris(self) -> UniqueUris {
        UniqueUris { uris: vec![self] }
    }
}
