use crate::{
    core::scalar::Scalar,
    dimension::Unit,
    symbol::Symbol,
    system::{System, SystemId},
};

/* ---------------------------------- ENUMS --------------------------------- */

#[derive(Clone, Copy, PartialEq)]
pub struct Variable {
    value: Value,
    symbol: Symbol,
    system: SystemId,
}

#[derive(Clone, Copy, PartialEq)]
enum Value {
    Known(Scalar),
    Unknown(Scalar),
}

/* --------------------------------- STRUCTS -------------------------------- */

pub struct VariableBuilder {
    name: String,
    guess: Option<Scalar>,
    value: Option<Scalar>,
    unit: Unit,
    system: SystemId,
    description: String,
}

impl Variable {
    pub fn builder<S: System>(name: impl Into<String>) -> VariableBuilder {
        VariableBuilder {
            name: name.into(),
            system: S::id(),
            guess: None,
            value: None,
            unit: Unit::Unitless,
            description: String::new(),
        }
    }

    pub fn symbol(&self) -> Symbol {
        self.symbol
    }

    pub fn as_unknown(&self) -> Option<Scalar> {
        match self.value {
            Value::Unknown(guess) => Some(guess),
            Value::Known(..) => None,
        }
    }

    pub fn as_known(&self) -> Option<Scalar> {
        match self.value {
            Value::Unknown(..) => None,
            Value::Known(value) => Some(value),
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
            "Variable must have either `guess` or `value`, not both. Values for knowns, guesses for unknowns"
        );

        let symbol = Symbol::new(&self.name)
            .set_unit(self.unit)
            .set_description(self.description);

        let value = match (self.guess, self.value) {
            (Some(guess), None) => Value::Unknown(guess),
            (None, Some(value)) => Value::Known(value),
            _ => unreachable!(),
        };

        Variable { value, symbol, system: self.system }
    }
}
