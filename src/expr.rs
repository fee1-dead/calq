use core::fmt;
use std::convert::Infallible;
use std::ops::{Add, Mul, Neg, Sub};

use bigdecimal::num_bigint::BigInt;
use bigdecimal::BigDecimal;
use chumsky::prelude::*;
use num_rational::BigRational;

use crate::div::CheckedDiv;

pub enum Value {
    Exact(BigRational),
    Decimal(BigDecimal),
}

fn perform_op<E>(
    a: Value,
    b: Value,
    do_exact: fn(BigRational, BigRational) -> Result<BigRational, E>,
    do_decimal: fn(BigDecimal, BigDecimal) -> Result<BigDecimal, E>,
) -> Result<Value, E> {
    let conv = |x: BigRational| {
        let (numer, denom) = x.into();
        BigDecimal::new(numer, 0) / BigDecimal::new(denom, 0)
    };
    match (a, b) {
        (Value::Exact(a), Value::Exact(b)) => do_exact(a, b).map(Value::Exact),
        (Value::Decimal(a), Value::Exact(b)) => do_decimal(a, conv(b)).map(Value::Decimal),
        (Value::Exact(a), Value::Decimal(b)) => do_decimal(conv(a), b).map(Value::Decimal),
        (Value::Decimal(a), Value::Decimal(b)) => do_decimal(a, b).map(Value::Decimal),
    }
}

macro_rules! binop_impl {
    (impl $Tr:ident for Value via $name:ident($op:tt)) => {
        impl $Tr for Value {
            type Output = Value;
            fn $name(self, rhs: Value) -> Value {
                perform_op::<Infallible>(self, rhs, |a, b| Ok(a $op b), |a, b| Ok(a $op b)).unwrap_or_else(|e| match e {})
            }
        }
    };
}

binop_impl!(impl Add for Value via add(+));
binop_impl!(impl Sub for Value via sub(-));
binop_impl!(impl Mul for Value via mul(*));

impl CheckedDiv for Value {
    type Target = Value;
    fn checked_div(self, other: Self) -> Result<Self::Target, crate::div::DivisionByZero> {
        perform_op(
            self,
            other,
            CheckedDiv::checked_div,
            CheckedDiv::checked_div,
        )
    }
}

impl Neg for Value {
    type Output = Value;
    fn neg(self) -> Self::Output {
        match self {
            Value::Exact(e) => Value::Exact(-e),
            Value::Decimal(d) => Value::Decimal(-d),
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
    Sub(Box<(Expr, Expr)>),
    Mul(Box<(Expr, Expr)>),
    Div(Box<(Expr, Expr)>),
    Neg(Box<Expr>),
}

impl Expr {
    pub fn eval(self) -> color_eyre::Result<Value> {
        Ok(match self {
            Expr::Value(val) => val,
            Expr::Add(values) => {
                let (a, b) = *values;
                a.eval()? + b.eval()?
            }
            Expr::Sub(values) => {
                let (a, b) = *values;
                a.eval()? - b.eval()?
            }
            Expr::Mul(values) => {
                let (a, b) = *values;
                a.eval()? * b.eval()?
            }
            Expr::Div(values) => {
                let (a, b) = *values;
                a.eval()?.checked_div(b.eval()?)?
            }
            Expr::Neg(neg) => -neg.eval()?,
        })
    }
}

pub fn expr_parser() -> impl Parser<char, Expr, Error = Simple<char>> {
    let int = text::int(10)
        .then(just('.').ignore_then(text::digits(10).or_not()).or_not())
        .then(just('e').ignore_then(text::int(10)).or_not())
        .try_map(|((int, dec), exp), span| {
            Ok(Expr::Value(if dec.is_some() || exp.is_some() {
                Value::Decimal({
                    let mut s = int.to_string();
                    if let Some(Some(dec)) = dec {
                        s.push('.');
                        s.push_str(&dec);
                    }
                    if let Some(exp) = exp {
                        s.push('e');
                        s.push_str(&exp);
                    }
                    s.parse::<BigDecimal>()
                        .map_err(|e| Simple::custom(span, e.to_string()))?
                })
            } else {
                Value::Exact(BigRational::new(
                    int.parse::<BigInt>()
                        .map_err(|e| Simple::custom(span, e.to_string()))?,
                    1.into(),
                ))
            }))
        })
        .padded();

    let op = |c| just(c).padded();

    let unary = op('-')
        .repeated()
        .then(int)
        .foldr(|_op, rhs| Expr::Neg(Box::new(rhs)));

    let product = unary
        .clone()
        .then(
            op('*')
                .to(Expr::Mul as fn(_) -> _)
                .or(op('/').to(Expr::Div as fn(_) -> _))
                .then(unary)
                .repeated(),
        )
        .foldl(|lhs, (op, rhs)| op(Box::new((lhs, rhs))));

    let sum = product
        .clone()
        .then(
            op('+')
                .to(Expr::Add as fn(_) -> _)
                .or(op('-').to(Expr::Sub as fn(_) -> _))
                .then(product)
                .repeated(),
        )
        .foldl(|lhs, (op, rhs)| op(Box::new((lhs, rhs))));

    sum.then_ignore(end())
}
