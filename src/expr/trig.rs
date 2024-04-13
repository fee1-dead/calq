use std::sync::LazyLock;

use bigdecimal::num_bigint::BigInt;
use bigdecimal::{BigDecimal, Num, One, RoundingMode, Signed, Zero};

macro_rules! lazy_constant {
    (static $name:ident = $($int_part:literal)? . $dec_part:literal) => {
        static $name: LazyLock<BigDecimal> = LazyLock::new(|| {
            const SCALE: i64 = $dec_part.len() as i64;
            BigDecimal::new(BigInt::from_str_radix(concat!($($int_part,)? $dec_part), 10).unwrap(), SCALE)
        });
    };
}

// http://www.hvks.com/Numerical/papers.html

// `N[Pi,100]` on wolfram cloud.
lazy_constant!(static PI = "3"."141592653589793238462643383279502884197169399375105820974944592307816406286208998628034825342117068");

// `N[Pi/2,100]` on wolfram cloud.
lazy_constant!(static PI_OVER_TWO = "1"."570796326794896619231321691639751442098584699687552910487472296153908203143104499314017412671058534");

// `N[Pi/4,100]` on wolfram cloud.
lazy_constant!(static PI_OVER_FOUR = ."7853981633974483096156608458198757210492923498437764552437361480769541015715522496570087063355292670");

#[test]
fn test_pi() {
    assert_eq!("3.14", PI.round(2).to_string());
    assert_eq!("1.57", PI_OVER_TWO.round(2).to_string());
    assert_eq!("0.79", PI_OVER_FOUR.round(2).to_string());
}

macro_rules! coeff_list {
    ({$($lit:literal),*}) => {
        &[
            $({
                static S: LazyLock<BigDecimal> = LazyLock::new(|| BigDecimal::from_str_radix(stringify!($lit),10).unwrap());
                &S
            }),*
        ]
    };
}

// https://math.stackexchange.com/questions/1344627/how-to-use-chebyshev-polynomials-to-approximate-sinx-and-cosx-within-t

/// Using Chebychev polynomials to compute sin in the domain [0, Pi/2]
/// Code: `AccountingForm[CoefficientList[N[-D[BesselJ[0,1]+2Sum[(-1)^n BesselJ[2n,1]ChebyshevT[2n,x],{n,10}],x]//Expand,20],x][[2;;;;2]],NumberSigns->{"-", ""}]`
fn sin_restricted(x: BigDecimal) -> BigDecimal {
    // list of coefficients for x^1, x^3, x^5, so on
    static COEFFS: &[&'static LazyLock<BigDecimal>] = coeff_list!({
        // 1.000000000000000000,
        -0.16666666666666666667,
        0.0083333333333333333330,
        -0.00019841269841269840864,
        0.0000027557319223985653805,
        -0.000000025052108385359014909,
        0.00000000016059043818789573688,
        -0.00000000000076471612574465163742,
        0.0000000000000028112496468372364173,
        -0.0000000000000000081233245813335029845
    });
    let x_squared = &x * &x;
    let mut current_mul = x.clone();
    let mut out = x;
    for coeff in COEFFS {
        current_mul *= &x_squared;
        out += &***coeff * &current_mul;
    }
    out
}

/// Using Chebychev polynomials to compute cos in the domain [0, Pi/2]
/// Code: `AccountingForm[CoefficientList[N[BesselJ[0,1]+2Sum[(-1)^n BesselJ[2n,1]ChebyshevT[2n,x],{n,10}]//Expand,20],x][[;;;;2]],NumberSigns->{"-", ""}]`
fn cos_restricted(x: BigDecimal) -> BigDecimal {
    // list of coefficients for x^0, x^2, x^4, so on
    static COEFFS: &[&'static LazyLock<BigDecimal>] = coeff_list!({
        // 1.000000000000000000,
        -0.50000000000000000000,
        0.041666666666666666667,
        -0.0013888888888888888888,
        0.000024801587301587301080,
        -0.00000027557319223985653805,
        0.0000000020876756987799179091,
        -0.000000000011470745584849695492,
        0.000000000000047794757859040727339,
        -0.00000000000000015618053593540202318,
        0.00000000000000000040616622906667514923
    });

    let x_squared = &x * &x;
    let mut current_mul = BigDecimal::one();
    let mut out = BigDecimal::one();
    for coeff in COEFFS {
        current_mul *= &x_squared;
        out += &***coeff * &current_mul;
    }
    out
}

// https://en.wikipedia.org/wiki/List_of_trigonometric_identities
pub fn sin(mut x: BigDecimal) -> BigDecimal {
    let mut was_negative = x.is_negative();
    // sin(-x) = sin(x), domain is now [0, ∞)
    if was_negative {
        x = -x;
    }

    // find out how many multiples of pi this has
    let (multiples_of_pi, exp) = (&x / &*PI)
        .with_scale_round(0, RoundingMode::Down)
        .into_bigint_and_exponent();
    debug_assert_eq!(0, exp);

    let multiples_of_pi_mod_two: BigInt = &multiples_of_pi % 2;
    if !multiples_of_pi_mod_two.is_zero() {
        // is odd, sin(pi + x) = -sin(x)
        was_negative = !was_negative;
    }

    // now we are at [0, pi]
    x -= &*PI * BigDecimal::new(multiples_of_pi, 0);

    // now at [0, pi/2]
    if &x > &*PI_OVER_TWO {
        x = &*PI - x;
    }

    let val = if &x > &*PI_OVER_FOUR {
        cos_restricted(&*PI_OVER_TWO - &x)
    } else {
        sin_restricted(x)
    };

    if was_negative {
        -val
    } else {
        val
    }
}

#[test]
fn test_sin() {
    assert_eq!(
        "0.19260484050094251300",
        sin("114514".parse().unwrap()).round(20).to_string()
    );
}
