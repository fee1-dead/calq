use core::fmt;
use std::convert::Infallible;
use std::ops::Neg;

use chumsky::prelude::*;
use rug::float::Round;
use rug::ops::{CompleteRound, DivAssignRound};
use rug::{Complete, Float, Integer, Rational};

use crate::div::CheckedDiv;

mod print;
// mod trig;

pub enum Value {
    Exact(Rational),
    Decimal(Float),
}

fn perform_op<E>(
    a: Value,
    b: Value,
    do_exact: fn(Rational, Rational) -> Result<Rational, E>,
    do_decimal: fn(Float, Float, &Evaluator) -> Result<Float, E>,
    evaluator: &Evaluator,
) -> Result<Value, E> {
    let conv = |x: Rational| {
        Float::with_val_round(evaluator.precision(), x, evaluator.round()).0
    };
    match (a, b) {
        (Value::Exact(a), Value::Exact(b)) => do_exact(a, b).map(Value::Exact),
        (Value::Decimal(a), Value::Exact(b)) => do_decimal(a, conv(b), evaluator).map(Value::Decimal),
        (Value::Exact(a), Value::Decimal(b)) => do_decimal(conv(a), b, evaluator).map(Value::Decimal),
        (Value::Decimal(a), Value::Decimal(b)) => do_decimal(a, b, evaluator).map(Value::Decimal),
    }
}

macro_rules! op_impl {
    ($name:ident($op:tt)) => {
        fn $name(self, other: Value, e: &Evaluator) -> Value {
            match perform_op::<Infallible>(self, other, |a, b| Ok(a $op b), |a, b, _| Ok(a $op b), e) {
                Ok(x) => x,
                Err(e) => match e {}
            }
        }
    };
}

impl Value {
    op_impl!(add(+));
    op_impl!(sub(-));
    op_impl!(mul(*));
    fn div(self, other: Value, evaluator: &Evaluator) -> Result<Value, crate::div::DivisionByZero> {
        perform_op(self, other, CheckedDiv::checked_div, |mut a, b, e| {
            a.div_assign_round(b, e.round());
            Ok(a)
        }, evaluator)
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

pub enum PrecisionMode {
    Decent,
}

impl PrecisionMode {
    fn precision(&self) -> u32 {
        match self {
            PrecisionMode::Decent => 100,
        }
    }
}

pub struct Evaluator {
    precision: PrecisionMode,
}

impl Default for Evaluator {
    fn default() -> Self {
        Evaluator { precision: PrecisionMode::Decent }
    }
}

impl Evaluator {
    fn round(&self) -> Round {
        Round::Nearest
    }
    fn precision(&self) -> u32 {
        self.precision.precision()
    }
    fn complete<C: CompleteRound<Prec = u32, Round = Round>>(&self, c: C) -> C::Completed {
        c.complete_round(self.precision(), self.round()).0
    }
    // evaluate a binary operation implemented numerically.
    fn eval_binop(
        &mut self,
        a: Expr,
        b: Expr,
        numerical: fn(Value, Value, &mut Evaluator) -> color_eyre::Result<Value>,
        fallback: fn(Expr, Expr) -> Expr,
    ) -> color_eyre::Result<Expr> {
        match (self.eval(a)?, self.eval(b)?) {
            (Expr::Value(a), Expr::Value(b)) => numerical(a, b, self).map(Expr::Value),
            (a, b) => Ok(fallback(a, b)),
        }
    }

    pub fn eval(&mut self, e: Expr) -> color_eyre::Result<Expr> {
        Ok(match e {
            Expr::Value(val) => Expr::Value(val),
            Expr::Symbol(s) => Expr::Symbol(s),
            Expr::Add(values) => {
                let (a, b) = *values;
                self.eval_binop(a, b, |a, b, e| Ok(a.add(b, &*e)), |a, b| Expr::Add(Box::new((a, b))))?
            }
            Expr::Sub(values) => {
                let (a, b) = *values;
                self.eval_binop(a, b, |a, b, e| Ok(a.sub(b, &*e)), |a, b| Expr::Sub(Box::new((a, b))))?
            }
            Expr::Mul(values) => {
                let (a, b) = *values;
                self.eval_binop(a, b, |a, b, e| Ok(a.mul(b, &*e)), |a, b| Expr::Mul(Box::new((a, b))))?
            }
            Expr::Div(values) => {
                let (a, b) = *values;
                self.eval_binop(
                    a,
                    b,
                    |a, b, e| a.div(b, &*e).map_err(Into::into),
                    |a, b| Expr::Div(Box::new((a, b))),
                )?
            }
            Expr::Neg(neg) => match self.eval(*neg)? {
                Expr::Value(v) => Expr::Value(-v),
                other => Expr::Neg(Box::new(other)),
            },
            Expr::Apply(left, args) => match self.eval(*left)? {
                Expr::Symbol(n) if n == "sin" && args.len() == 1 => {
                    let arg = args.into_iter().next().unwrap();
                    match self.eval(arg)? {
                        Expr::Value(Value::Decimal(mut d)) => Expr::Value(Value::Decimal({
                            d.sin_round(self.round()); d
                        })),
                        _ => todo!(),
                    }
                }
                _ => todo!(),
            },
        })
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

pub fn expr_parser(e: &Evaluator) -> impl Parser<char, Expr, Error = Simple<char>> + '_ {
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
                        let f = Float::parse(s).map_err(|e| Simple::custom(span, e.to_string()))?;
                        e.complete(f)
                    })
                } else {
                    Value::Exact(Integer::parse(int).map_err(|e| Simple::custom(span, e.to_string()))?.complete().into())
                }))
            })
            .padded();

        let atom = int
            .or(expr.clone().delimited_by(just('('), just(')')))
            .or(text::ident().map(Expr::Symbol))
            .padded();

        let op = |c| just(c).padded();

        let func = atom
            .clone()
            .then(
                expr.separated_by(just(','))
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
