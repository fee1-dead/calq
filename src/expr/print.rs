use std::fmt::{self, Write};

use super::{Expr, PrecedenceContext, Value};

/* pub fn print_expr_to_string(x: &Expr) -> String {
    let mut p = Printer::new_string();
    p.print(x).expect("String format does not have errors");
    p.into_inner()
} */

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Printer {
            writer: f,
            round_digits: 8,
        }
        .print(self)
    }
}

pub struct Printer<W: Write> {
    writer: W,
    round_digits: usize,
}

/*
fn is_denominator(x: &Expr) -> bool {
    /* if let Expr::Pow(x) = x
    && let Expr::Value(x) = &x.1
    && x.is_negative() {
        true
    } else {
        false
    } */
    false
}*/

/*
fn can_combine_with_next(x: &Expr) -> bool {
    match x {
        Expr::Value(_)
        | Expr::Symbol(_)
        | Expr::Add(_)
        | Expr::Sub(_)
        // | Expr::Pow(_)
        | Expr::Apply(..) => true,
        Expr::Mul(x) => can_combine_with_next(&x.1),
        // Expr::Factorial(_) => false,
    }
}

fn can_combine_with_prev(x: &Expr) -> bool {
    match x {
        Expr::Mul(x) => x.first().map_or(true, can_combine_with_prev),
        // Expr::Pow(_)
        | Expr::Symbol(_)
        | Expr::Add(_)
        | Expr::Apply(..) => true,
        /* Expr::Factorial(_) | */ Expr::Value(_) => false,
    }
}
*/

impl<W: Write> Printer<W> {
    pub fn print(&mut self, x: &Expr) -> fmt::Result {
        self.print_with_precedence(x, PrecedenceContext::NoPrecedence)
    }

    pub fn maybe_enter_parens(
        &mut self,
        f: impl FnOnce(&mut Self) -> fmt::Result,
        parens: bool,
    ) -> fmt::Result {
        if parens {
            self.writer.write_char('(')?;
        }
        f(self)?;
        if parens {
            self.writer.write_char(')')?;
        }
        Ok(())
    }

    /*pub fn enter_parens(&mut self, f: impl FnOnce(&mut Self) -> fmt::Result) -> fmt::Result {
        self.writer.write_char('(')?;
        f(self)?;
        self.writer.write_char(')')
    }*/

    /*pub fn print_product(
        &mut self,
        exprs: &[Expr],
        p: PrecedenceContext,
    ) -> fmt::Result {
        self.maybe_enter_parens(
            |this| {
                if exprs.iter().any(is_denominator) {
                    write!(this.writer, "frac(")?;

                    let mut denom = Printer::new_string();
                    let mut num_empty = true;
                    let mut num_prev_can_combine = false;
                    let mut denom_prev_can_combine = false;
                    for exp in exprs {
                        if is_denominator(exp) {
                            let Expr::Pow(x) = exp else {
                                panic!("expected power")
                            };

                            if !denom.writer.is_empty() {
                                // TODO: if numeric power is not one, then we would print a power,
                                // which means it would always be able to combine with previous.
                                if denom_prev_can_combine && can_combine_with_prev(exp) {
                                    denom.writer.push(' ');
                                } else {
                                    denom.writer.push_str(" dot.op ");
                                }
                            }

                            denom_prev_can_combine = can_combine_with_next(&x.0);

                            denom.print_with_precedence(&x.0, PrecedenceContext::Mul)?;

                            // TODO detect (x)^(-y)
                            if let Expr::Value(x) = &x.1 {
                                let abs = x.abs();
                                if !abs.is_one() {
                                    denom.writer.push_str("^(");
                                    denom.print_constant(&abs)?;
                                    denom.writer.push(')');
                                    denom_prev_can_combine = true;
                                }
                            }
                        } else {
                            if !num_empty {
                                if num_prev_can_combine && can_combine_with_prev(exp) {
                                    this.writer.write_char(' ')?;
                                } else {
                                    this.writer.write_str(" dot.op ")?;
                                }

                                num_empty = false;
                            }
                            num_prev_can_combine = can_combine_with_next(exp);
                            this.print_with_precedence(exp, PrecedenceContext::Mul)?;
                        }
                    }
                    let denom = denom.into_inner();
                    write!(this.writer, ",{denom})")?;
                } else {
                    let mut prev_can_combine = false;
                    for (n, item) in exprs.iter().enumerate() {
                        if n != 0 {
                            if prev_can_combine && can_combine_with_prev(item) {
                                this.writer.write_char(' ')?;
                            } else {
                                this.writer.write_str(" dot.op ")?;
                            }
                        }
                        prev_can_combine = can_combine_with_next(item);
                        this.print_with_precedence(item, PrecedenceContext::Mul)?;
                    }
                }
                Ok(())
            },
            PrecedenceContext::Mul < p,
        )
    } */

    pub fn print_with_precedence(&mut self, x: &Expr, p: PrecedenceContext) -> fmt::Result {
        let new_ctxt = x.precedence();
        match x {
            /*Expr::Factorial(x) => {
                self.print_with_precedence(x, new_ctxt)?;
                write!(self.writer, "!")?;
            }*/
            Expr::Value(x) => match x {
                Value::Decimal(dec) => {
                    let (sign, mut string, exp) = dec.to_sign_string_exp(10, Some(self.round_digits));
                    let sign = if sign {
                        ""
                    } else {
                        "-"
                    };
                    let exp = exp.map(|x| x - 1);
                    let (prefix, string, suffix, suffix2) = match exp {
                        Some(exp @ ..=-4) => {
                            let rest = string.split_off(1);
                            (string, ".".into(), rest, format!("e{exp}"))
                        }
                        Some(exp @ -5..=-1) => {
                            (format!("0.{}", "0".repeat((-exp - 1) as usize)), string, String::new(), String::new())
                        }
                        Some(0) => {
                            let rest = string.split_off(1);
                            (string, ".".into(), rest, String::new())
                        }
                        // TODO fix below, that doesn't work!!
                        Some(exp @ 1..) => {
                            (String::new(), string, "0".repeat(exp as usize), String::new())
                        }
                        None => (String::new(), string, String::new(), String::new()),
                    };
                    
                    write!(self.writer, "{sign}{prefix}{string}{suffix}{suffix2}")?;
                }
                Value::Exact(e) => {
                    write!(self.writer, "{e}")?;
                }
            },
            Expr::Symbol(x) => {
                self.writer.write_str(x)?;
            }
            Expr::Neg(x) => {
                write!(self.writer, "-")?;
                self.print_with_precedence(x, new_ctxt)?;
            }
            Expr::Add(exprs) | Expr::Mul(exprs) | Expr::Div(exprs) | Expr::Sub(exprs) => {
                let s = match x {
                    Expr::Add(_) => "+",
                    Expr::Mul(_) => "*",
                    Expr::Div(_) => "/",
                    Expr::Sub(_) => "-",
                    _ => unreachable!(),
                };

                self.maybe_enter_parens(
                    |this| {
                        this.print_with_precedence(&exprs.0, new_ctxt)?;
                        this.writer.write_str(s)?;
                        this.print_with_precedence(&exprs.1, new_ctxt)?;
                        Ok(())
                    },
                    new_ctxt < p,
                )?;
            }
            /* Expr::Pow(x) => {
                let (base, exp) = &**x;
                let base_ctxt = base.precedence_ctxt();
                self.maybe_enter_parens(
                    |this| this.print_with_precedence(base, PrecedenceContext::NoPrecedence),
                    base_ctxt != PrecedenceContext::NoPrecedence
                        && base_ctxt < PrecedenceContext::Pow,
                )?;
                self.writer.write_char('^')?;
                self.enter_parens(|this| {
                    this.print_with_precedence(exp, PrecedenceContext::NoPrecedence)
                })?;
            } */
            Expr::Apply(name, params) => {
                write!(self.writer, "\"{name}\"(")?;
                for (n, param) in params.iter().enumerate() {
                    if n != 0 {
                        self.writer.write_char(',')?;
                    }

                    self.print_with_precedence(param, PrecedenceContext::NoPrecedence)?;
                }
                self.writer.write_char(')')?;
            }
        }
        Ok(())
    }

    /*pub fn into_inner(self) -> W {
        self.writer
    }*/
}

/*
impl Printer<String> {
    pub fn new_string() -> Self {
        Self {
            writer: String::new(),
        }
    }
}*/
