use core::fmt;
use std::ops::Add;

use bigdecimal::BigDecimal;
use chumsky::prelude::*;
use num_rational::BigRational;

pub enum Value {
    Exact(BigRational),
    Decimal(BigDecimal),
}

impl Add for Value {
    type Output = Value;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Value::Exact(a), Value::Exact(b)) => Value::Exact(a + b),
            (Value::Decimal(d), Value::Exact(e)) | (Value::Exact(e), Value::Decimal(d)) => {
                let (numer, denom) = e.into();
                let d2 = BigDecimal::new(numer, 0) / BigDecimal::new(denom, 0);
                Value::Decimal(d + d2)
            }
            (Value::Decimal(a), Value::Decimal(b)) => Value::Decimal(a + b),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Exact(r) => r.fmt(f),
            // TODO print in scientific notation when possible
            Value::Decimal(d) => d.fmt(f),
        }
    }
}

pub enum Expr {
    Value(Value),
    Add(Box<(Expr, Expr)>),
}

impl Expr {
    pub fn eval(self) -> Value {
        match self {
            Expr::Value(val) => val,
            Expr::Add(values) => {
                let (a, b) = *values;
                a.eval() + b.eval()
            }
        }
    }
}

pub fn expr_parser() -> impl Parser<char, Expr, Error = Simple<char>> {
    // TODO add syntax like 1e10.
    let int = text::int(10)
        .map(|s: String| Expr::Value(Value::Exact(BigRational::new(s.parse().unwrap(), 1.into()))))
        .padded();
    // TODO add unary `-`

    let op = |c| just(c).padded();

    let sum = int.clone().then(op('+').to(Expr::Add as fn(_) -> _).then(int).repeated()).foldl(|lhs, (op, rhs)| {
        op(Box::new((lhs, rhs)))
    });

    sum.then_ignore(end())
}
