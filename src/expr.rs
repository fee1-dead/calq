use core::fmt;
use std::convert::Infallible;
use std::ops::{Add, Mul, Neg, Sub};

use bigdecimal::num_bigint::BigInt;
use bigdecimal::BigDecimal;
use chumsky::prelude::*;
use num_rational::BigRational;

use crate::div::CheckedDiv;

mod print;
mod trig;

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
    Symbol(String),
    Add(Box<(Expr, Expr)>),
    Sub(Box<(Expr, Expr)>),
    Mul(Box<(Expr, Expr)>),
    Div(Box<(Expr, Expr)>),
    Neg(Box<Expr>),
    Apply(Box<Expr>, Vec<Expr>),
}

impl Expr {
    fn eval_binop(
        a: Expr,
        b: Expr,
        numerical: fn(Value, Value) -> color_eyre::Result<Value>,
        fallback: fn(Expr, Expr) -> Expr,
    ) -> color_eyre::Result<Expr> {
        match (a.eval()?, b.eval()?) {
            (Expr::Value(a), Expr::Value(b)) => numerical(a, b).map(Expr::Value),
            (a, b) => Ok(fallback(a, b)),
        }
    }
    pub fn eval(self) -> color_eyre::Result<Expr> {
        Ok(match self {
            Expr::Value(val) => Expr::Value(val),
            Expr::Symbol(s) => Expr::Symbol(s),
            Expr::Add(values) => {
                let (a, b) = *values;
                Self::eval_binop(a, b, |a, b| Ok(a + b), |a, b| Expr::Add(Box::new((a, b))))?
            }
            Expr::Sub(values) => {
                let (a, b) = *values;
                Self::eval_binop(a, b, |a, b| Ok(a - b), |a, b| Expr::Sub(Box::new((a, b))))?
            }
            Expr::Mul(values) => {
                let (a, b) = *values;
                Self::eval_binop(a, b, |a, b| Ok(a * b), |a, b| Expr::Mul(Box::new((a, b))))?
            }
            Expr::Div(values) => {
                let (a, b) = *values;
                Self::eval_binop(
                    a,
                    b,
                    |a, b| a.checked_div(b).map_err(Into::into),
                    |a, b| Expr::Div(Box::new((a, b))),
                )?
            }
            Expr::Neg(neg) => match neg.eval()? {
                Expr::Value(v) => Expr::Value(-v),
                other => Expr::Neg(Box::new(other)),
            },
            Expr::Apply(left, args) => match left.eval()? {
                Expr::Symbol(n) if n == "sin" && args.len() == 1 => {
                    let arg = args.into_iter().next().unwrap();
                    match arg.eval()? {
                        Expr::Value(Value::Decimal(d)) => Expr::Value(Value::Decimal(trig::sin(d))),
                        _ => todo!(),
                    }
                }
                _ => todo!()
            },
        })
    }
}

pub fn expr_parser() -> impl Parser<char, Expr, Error = Simple<char>> {
    let expr = recursive(|expr| {
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

        let atom = int
            .or(expr.clone().delimited_by(just('('), just(')')))
            .or(text::ident().map(Expr::Symbol))
            .padded();

        let op = |c| just(c).padded();

        let func = atom.clone()
            .then(
                expr
                    .separated_by(just(','))
                    .allow_trailing() // Foo is Rust-like, so allow trailing commas to appear in arg lists
                    .delimited_by(just('('), just(')')),
            )
            .map(|(f, args)| Expr::Apply(Box::new(f), args));

        let calls = func.or(atom);

        let unary = op('-')
            .repeated()
            .then(calls)
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

        sum
    });
    expr.then_ignore(end())
}

/// An enum representing operator precedence. Useful for printing stuff.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrecedenceContext {
    /// Has no precedence. (wrapped in parens or function args)
    NoPrecedence,
    /// Sum context, currently equivalent to `NoPrecedence`
    Sum,
    /// Product context. Sums must be wrapped in parens
    Product,
    /// Exponentiation
    // Pow,
    /// Negation
    Neg,
    /// These operations are performed to their immediate left, so if their left
    /// is a compound expression we certainly want to wrap them in parenthesis.
    FunctionOrFactorial,
}

impl Expr {
    pub fn precedence(&self) -> PrecedenceContext {
        use PrecedenceContext::*;
        match self {
            Self::Value(_) | Self::Symbol(_) => NoPrecedence,
            Self::Mul(_) | Self::Div(_) => Product,
            Self::Add(_) | Self::Sub(_) => Sum,
            Self::Neg(_) => Neg,
            Self::Apply(..) => FunctionOrFactorial,
            //Self::Pow(_) => Pow,
            // Self::Factorial(_) | Self::Function(_, _) => FunctionOrFactorial,
        }
    }
}
