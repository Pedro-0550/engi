use crate::units::Dimension;

pub const DIMENSIONLESS: Dimension =
    Dimension { T: 0, L: 0, M: 0, I: 0, Θ: 0, J: 0, N: 0 };

// SI base dimensions

pub const TIME: Dimension =
    Dimension { T: 1, L: 0, M: 0, I: 0, Θ: 0, J: 0, N: 0 };

pub const LENGTH: Dimension =
    Dimension { T: 0, L: 1, M: 0, I: 0, Θ: 0, J: 0, N: 0 };

pub const MASS: Dimension =
    Dimension { T: 0, L: 0, M: 1, I: 0, Θ: 0, J: 0, N: 0 };

pub const ELECTRIC_CURRENT: Dimension =
    Dimension { T: 0, L: 0, M: 0, I: 1, Θ: 0, J: 0, N: 0 };

pub const TEMPERATURE: Dimension =
    Dimension { T: 0, L: 0, M: 0, I: 0, Θ: 1, J: 0, N: 0 };

pub const LUMINOUS_INTENSITY: Dimension =
    Dimension { T: 0, L: 0, M: 0, I: 0, Θ: 0, J: 1, N: 0 };

pub const AMOUNT_OF_SUBSTANCE: Dimension =
    Dimension { T: 0, L: 0, M: 0, I: 0, Θ: 0, J: 0, N: 1 };
