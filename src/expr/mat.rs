use std::{
    num::NonZero,
    ops::{Index, IndexMut},
};

use crate::expr::{Expr, Node, shape::Shape};

/// Row-major matrix type
#[derive(PartialEq, Clone, Debug, Hash, Eq)]
pub struct Matrix {
    shape: Shape,
    elements: Box<[Node]>,
}

impl Matrix {
    // pub fn from_fn(
    //     rows: impl Into<usize>,
    //     cols: impl Into<usize>,
    //     f: FnMut(usize, usize) -> Expr,
    // ) -> Matrix {
    // }

    pub fn zeros(rows: impl Into<usize>, cols: impl Into<usize>) -> Self {
        let rows = rows.into();
        let cols = cols.into();
        Self {
            shape: Shape::rect(rows, cols),
            elements: vec![0.into(); rows * cols].into_boxed_slice(),
        }
    }

    /// Returns (rows, cols) for this matrix
    pub fn shape(&self) -> Shape {
        self.shape
    }

    pub fn rows(&self) -> NonZero<usize> {
        self.shape.rows
    }

    pub fn cols(&self) -> NonZero<usize> {
        self.shape.cols
    }

    pub fn elements(&self) -> &[Node] {
        &self.elements
    }

    pub fn into_elements(self) -> Box<[Node]> {
        self.elements
    }

    pub fn map(&self, f: impl FnMut(&Node) -> Node) -> Matrix {
        Matrix {
            shape: self.shape,
            elements: self.elements.iter().map(f).collect(),
        }
    }

    pub fn into_map(self, f: impl FnMut(Node) -> Node) -> Matrix {
        Matrix {
            shape: self.shape,
            elements: self.elements.into_iter().map(f).collect(),
        }
    }
}

impl Index<usize> for Matrix {
    type Output = [Node];

    fn index(&self, row: usize) -> &Self::Output {
        let start = row * self.shape.cols.get();
        let end = start + self.shape.cols.get();
        &self.elements[start..end]
    }
}

impl IndexMut<usize> for Matrix {
    fn index_mut(&mut self, row: usize) -> &mut Self::Output {
        let start = row * self.shape.cols.get();
        let end = start + self.shape.cols.get();
        &mut self.elements[start..end]
    }
}
