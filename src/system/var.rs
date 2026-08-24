use crate::{core::scalar::Scalar, dimension::Unit, symbol::Symbol};

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Clone, Copy, PartialEq)]
pub enum Variable {
    Unknown { symbol: Symbol, guess: Scalar },
    Known { symbol: Symbol, value: Scalar },
}

/* --------------------------------- STRUCTS -------------------------------- */

pub struct VariableBuilder {
    name: String,
    guess: Option<Scalar>,
    value: Option<Scalar>,
    unit: Unit,
    description: String,
}

impl Variable {
    pub fn builder(name: impl Into<String>) -> VariableBuilder {
        VariableBuilder {
            name: name.into(),
            guess: None,
            value: None,
            unit: Unit::Unitless,
            description: String::new(),
        }
    }

    pub fn symbol(&self) -> Symbol {
        match self {
            Variable::Unknown { symbol, .. } => *symbol,
            Variable::Known { symbol, .. } => *symbol,
        }
    }

    pub fn as_unknown(&self) -> Option<Scalar> {
        match self {
            Variable::Unknown { guess, .. } => Some(*guess),
            Variable::Known { .. } => None,
        }
    }

    pub fn as_known(&self) -> Option<Scalar> {
        match self {
            Variable::Unknown { .. } => None,
            Variable::Known { value, .. } => Some(*value),
        }
    }
}

impl VariableBuilder {
    pub fn guess(mut self, guess: Scalar) -> Self {
        self.guess = Some(guess);
        self.value = None;
        self
    }

    pub fn value(mut self, value: Scalar) -> Self {
        self.value = Some(value);
        self.guess = None;
        self
    }

    pub fn unit(mut self, unit: Unit) -> Self {
        self.unit = unit;
        self
    }

    pub fn desc(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// TODO: Add bounds support to `Symbol`.
    ///
    /// For now this intentionally accepts any type and ignores it, so the
    /// public builder API can exist before the bounds representation is ready.
    pub fn bounds<B>(self, _bounds: B) -> Self {
        self
    }

    pub fn build(self) -> Variable {
        assert!(
            self.guess.is_some() ^ self.value.is_some(),
            "Variable must have exactly one of `guess` or `value`"
        );

        let symbol = Symbol::new(&self.name)
            .set_unit(self.unit)
            .set_description(self.description);

        match (self.guess, self.value) {
            (Some(guess), None) => Variable::Unknown { symbol, guess },
            (None, Some(value)) => Variable::Known { symbol, value },
            _ => unreachable!(),
        }
    }
}
