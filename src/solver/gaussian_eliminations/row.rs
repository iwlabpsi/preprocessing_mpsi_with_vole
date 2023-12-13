use scuttlebutt::field::FiniteField as FF;
use std::ops::{Index, IndexMut, Range};

pub(crate) struct Row<F: FF> {
    pub values: Vec<bool>,
    pub target: F,
}

impl<F: FF> Row<F> {
    pub fn new(values: Vec<bool>, target: F) -> Self {
        Self { values, target }
    }
}

impl<F: FF> Index<usize> for Row<F> {
    type Output = bool;
    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl<F: FF> IndexMut<usize> for Row<F> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.values[index]
    }
}

pub(crate) fn add_rows<F: FF>(
    matrix: &mut Vec<Row<F>>,
    added_row_idx: usize,
    add_row_idx: usize,
    range: Range<usize>,
) {
    for i in range {
        let val = matrix[add_row_idx].values[i];
        matrix[added_row_idx].values[i] ^= val;
    }
    let val = matrix[add_row_idx].target;
    matrix[added_row_idx].target += val;
}
