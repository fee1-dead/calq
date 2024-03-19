use std::error::Error;
use std::fmt::Display;

use bigdecimal::BigDecimal;
use num_rational::BigRational;

#[derive(Debug, Default)]
pub struct DivisionByZero;

impl Display for DivisionByZero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad("disivion by zero")
    }
}

impl Error for DivisionByZero {}

pub trait CheckedDiv<T = Self> {
    type Target;
    fn checked_div(self, other: T) -> Result<Self::Target, DivisionByZero>;
}

macro_rules! impl_checked_div {
    ($($t:ty),*$(,)?) => {$(
        impl CheckedDiv for $t {
            type Target = $t;
            fn checked_div(self, other: Self) -> Result<Self::Target, DivisionByZero> {
                if other == Default::default() {
                    Err(DivisionByZero)
                } else {
                    Ok(self / other)
                }
            }
        }
    )*};
}

impl_checked_div!(BigRational, BigDecimal);
