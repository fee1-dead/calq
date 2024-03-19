use bigdecimal::BigDecimal;

// TODO: this is quite imprecise
//
// https://math.stackexchange.com/questions/1344627/how-to-use-chebyshev-polynomials-to-approximate-sinx-and-cosx-within-t
pub fn sin(mut x: BigDecimal) -> BigDecimal {
    // -x^2
    let sq = -(&x * &x);
    let mut output = x.clone();
    for denom in [6, 120, 5040, 362880] {
        x *= &sq;
        output += &x / denom;
    }
    output
}
