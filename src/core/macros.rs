#[macro_export]
macro_rules! impl_binary_op {
    ($result:ty, $t0:ty, $ty:ty, $op:ident, $method:ident, $expr:expr, normal) => {
        impl $op<$ty> for $t0 {
            type Output = $result;

            fn $method(self, rhs: $ty) -> $result {
                $expr(self.into(), rhs.into()).into()
            }
        }
    };
    ($result:ty, $t0:ty, $ty:ty, $op:ident, $method:ident, $expr:expr, symmetrical) => {
        impl $op<$ty> for $t0 {
            type Output = $result;

            fn $method(self, rhs: $ty) -> $result {
                $expr(self.into(), rhs.into()).into()
            }
        }

        impl $op<$t0> for $ty {
            type Output = $result;

            fn $method(self, rhs: $t0) -> $result {
                $expr(self.into(), rhs.into()).into()
            }
        }
    };
}

#[macro_export]
macro_rules! impl_assign_op {
    ($t0:ty, $ty:ty, $op:ident, $method:ident, $expr:expr) => {
        impl $op<$ty> for $t0 {
            fn $method(&mut self, rhs: $ty) {
                $expr(self.into(), rhs.into());
            }
        }
    };
}

#[macro_export]
macro_rules! impl_as {
    ($ty:ident, $($variant:ident => $inner:ty),* $(,)?) => {
        paste::paste! {
            impl $ty {
                $(
                    pub fn [<as_ $variant:snake>](&self) -> Option<&$inner> {
                        match self {
                            Self::$variant(value) => Some(value),
                            _ => None,
                        }
                    }
                )*
            }
        }
    };
}
