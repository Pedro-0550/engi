use std::num::NonZero;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
pub struct Shape {
    pub rows: NonZero<usize>,
    pub cols: NonZero<usize>,
}

/* --------------------------------- TRAITS --------------------------------- */

pub trait Shaped {
    fn shape(&self) -> Shape;
}

impl From<(usize, usize)> for Shape {
    fn from(value: (usize, usize)) -> Self {
        Self::rect(value.0, value.1)
    }
}

impl Shape {
    // SAFETY:
    // As of August 2026, 1 is not equal to 0.
    // If this changes in the future, use checked version instead.
    pub const SCALAR: Self = unsafe {
        Shape {
            cols: NonZero::<usize>::new_unchecked(1),
            rows: NonZero::<usize>::new_unchecked(1),
        }
    };

    pub fn transpose(self) -> Self {
        Self { rows: self.cols, cols: self.rows }
    }

    pub fn square(size: usize) -> Self {
        Self { rows: size.try_into().unwrap(), cols: size.try_into().unwrap() }
    }

    pub fn rect(rows: usize, cols: usize) -> Self {
        Self { rows: rows.try_into().unwrap(), cols: cols.try_into().unwrap() }
    }

    pub fn is_scalar(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() == 1
    }

    pub fn is_row(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() == 1
    }

    pub fn is_col(&self) -> bool {
        self.rows.get() == 1 && self.cols.get() > 1
    }

    pub fn is_vec(&self) -> bool {
        (self.rows.get() > 1) ^ (self.cols.get() > 1)
    }

    pub fn is_rect_mat(&self) -> bool {
        self.rows.get() > 1 && self.cols.get() > 1
    }

    pub fn is_square_mat(&self) -> bool {
        self.rows.get() > 1 && self.rows == self.rows
    }
}
