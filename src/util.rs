use std::env;
use std::mem::swap;

use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn get_env_var(name: &str) -> &'static str {
    match env::var(name) {
        Ok(value) => Box::new(value).leak(),
        Err(_) => panic!("Environment variable not found: {name}"),
    }
}

pub trait Subdivision {
    fn subdivide(&self, max_chunk_size: usize) -> SubdivisionIter;
}

impl Subdivision for &str {
    fn subdivide(&self, max_chunk_size: usize) -> SubdivisionIter {
        SubdivisionIter::new(self, max_chunk_size)
    }
}

pub struct SubdivisionIter<'a> {
    source: &'a str,
    index: usize,
    max_chunk_size: usize,
}

impl<'a> SubdivisionIter<'a> {
    fn new(source: &'a str, max_chunk_size: usize) -> SubdivisionIter<'a> {
        SubdivisionIter {
            source,
            index: 0,
            max_chunk_size,
        }
    }
}

impl<'a> Iterator for SubdivisionIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.source.len() {
            return None;
        }

        let mut next_index = 0;
        for char in self.source[self.index..].chars() {
            if next_index + char.len_utf8() > self.max_chunk_size {
                break;
            }
            next_index += char.len_utf8();
        }
        let next_index = self.index + next_index;
        let result = self.source[self.index..next_index].trim();
        self.index = next_index;

        Some(result)
    }
}
