use std::{
    array, cell::Cell, collections::HashMap, hash::Hash, iter::once, ops::Mul,
    rc::Rc, sync::LazyLock, time::Duration,
};

use ahash::{AHashMap, AHashSet};
use itertools::Itertools;
use num::{One, Zero, complex::ComplexFloat};
use ordered_float::OrderedFloat;

use crate::{
    core::{
        arena::Arena,
        scalar::{Scalar, gcd_f64},
    },
    dimension::Quantity,
    expr::{
        Expr, Node,
        ops::{Binary, Pow, Unary, Variadic, cos, pow, sin, tan},
    },
    simplify::normal::Normalize,
    symbol::Symbol, // set::Set,
};

/* --------------------------------- MODULES -------------------------------- */

#[cfg(test)]
mod test;

pub mod normal;

/* --------------------------------- TRAITS --------------------------------- */

pub trait Simplify {
    fn simplify(&self, ctx: &mut SimplifyContext) -> Expr;
    // fn range(&self) -> Set;
}

/* -------------------------------- CONSTANTS ------------------------------- */

// const CACHE_CAPACITY: u64 = 4096;
// const CACHE: LazyLock<Cache<Expr, Expr>> = LazyLock::new(|| {
//     Cache::builder()
//         .initial_capacity(CACHE_CAPACITY as usize / 8)
//         .max_capacity(CACHE_CAPACITY)
//         .time_to_live(Duration::from_weeks(1))
//         .time_to_idle(Duration::from_weeks(1))
//         .build()
// });

/* --------------------------------- STRUCTS -------------------------------- */

enum SimplificationStep {
    GroupTerms {},
    FactorTerms {},
}

pub struct SimplifyContext {
    steps: Option<Vec<SimplificationStep>>,
    cache: HashMap<Expr, Expr>,
}

// struct Path {
//     expr: Expr,
//     cost: usize,
//     seen: HashSet<Expr>,
// }

/* ---------------------------------- IMPLS --------------------------------- */

impl SimplifyContext {
    pub fn new() -> Self {
        Self { steps: None, cache: HashMap::new() }
    }
}

impl Expr {
    pub(crate) fn simplify_inner(&self, ctx: &mut SimplifyContext) -> Self {
        if self.node().is_symbol() || self.node().is_const() {
            return self.clone();
        }

        if let Some(hit) = ctx.cache.get(self) {
            return hit.clone();
        }

        let simplified = match self.node() {
            Node::Variadic(variadic) => variadic.simplify(ctx),
            Node::Unary(single) => single.simplify(ctx),
            Node::Binary(double) => double.simplify(ctx),
            Node::Matrix(matrix) => todo!(),
            _ => unreachable!(),
        }
        .normalize(true);

        ctx.cache.insert(self.clone(), simplified.clone());
        simplified
    }
}

impl Simplify for Expr {
    fn simplify(&self, ctx: &mut SimplifyContext) -> Expr {
        if self.node().is_symbol() || self.node().is_const() {
            return self.clone();
        }

        let initial = self.normalize(true);
        let mut step = initial.clone();

        // if let Some(hit) = CACHE.get(&step) {
        //     return hit;
        // }

        loop {
            let simplified = step.simplify_inner(ctx);

            if simplified == step {
                break;
            }

            step = simplified;
        }

        // CACHE.insert(initial, step.clone());

        step
    }

    // fn range(&self) -> Set {
    //     todo!()
    // }
}

macro_rules! trig_simplify {
    ($inv:ident, $fn:ident, $expr:ident, $self:ident, $ctx:ident) => {
        match $expr.node() {
            Node::Const(qty) => Scalar::from(qty.value().$fn()).into(),
            Node::Unary(op) if let Unary::$inv(ref x) = *op => {
                x.simplify_inner($ctx)
            }
            _ => $self.with_arg($self.arg().simplify_inner($ctx)).into(),
        }
    };
}

impl Simplify for Unary {
    fn simplify(&self, ctx: &mut SimplifyContext) -> Expr {
        match self {
            Unary::Sin(x) => trig_simplify!(Asin, sin, x, self, ctx),
            Unary::Cos(x) => trig_simplify!(Acos, cos, x, self, ctx),
            Unary::Tan(x) => trig_simplify!(Atan, tan, x, self, ctx),
            Unary::Sinh(x) => trig_simplify!(Asinh, sinh, x, self, ctx),
            Unary::Cosh(x) => trig_simplify!(Acosh, cosh, x, self, ctx),
            Unary::Tanh(x) => trig_simplify!(Atanh, tanh, x, self, ctx),

            Unary::Transpose(expr) => todo!(),
            Unary::Conj(expr) => todo!(),
            Unary::Arg(expr) => todo!(),
            Unary::Det(expr) => todo!(),
            Unary::Norm(expr) => todo!(),
            _ => self.with_arg(self.arg().simplify_inner(ctx)).into(),
        }
    }
}

impl Simplify for Binary {
    fn simplify(&self, ctx: &mut SimplifyContext) -> Expr {
        let simplified = self
            .with_args(array::from_fn(|i| self.args()[i].simplify_inner(ctx)))
            .into();

        match &simplified {
            Binary::Pow(Pow { base, exp }) => {
                if let Node::Binary(Binary::Pow(Pow {
                    base: inner_base,
                    exp: inner_exp,
                })) = &base.node()
                    && let Node::Const(exp) = exp.node()
                    && let Node::Const(inner_exp) = inner_exp.node()
                    && exp.value().is_integer()
                    && inner_exp.value().is_integer()
                {
                    pow(inner_base, *exp * *inner_exp).simplify_inner(ctx)
                } else if let Node::Const(qty) = exp.node()
                    && qty.value().is_zero()
                {
                    (1.0).into()
                } else if let Node::Const(qty) = exp.node()
                    && qty.value().is_one()
                {
                    base.clone()
                } else if let Node::Const(qty) = base.node()
                    && qty.value().is_one()
                {
                    (1.0).into()
                } else {
                    simplified.into()
                }
            }
            _ => simplified.into(),
        }
    }
}

impl Simplify for Variadic {
    fn simplify(&self, ctx: &mut SimplifyContext) -> Expr {
        let simplified =
            self.operands().iter().map(|expr| expr.simplify_inner(ctx));

        let mut groupings =
            AHashMap::<Expr, Scalar>::with_capacity(self.operands().len());

        for term in simplified {
            /* -------------------------------------------------------------------------- */
            // joins coefficients: 2x + x -> 3x
            if self.is_add()
                && let Node::Variadic(Variadic::Mul(terms)) = term.node()
            {
                let (coef, exprs) = extract_const(&terms);
                *groupings
                    .entry(Variadic::Mul(exprs.collect()).normalize(false))
                    .or_insert(0.0.into()) +=
                    coef.unwrap_or(1.0.into()).value();
            /* -------------------------------------------------------------------------- */
            // joins integer powers -> x^2 * x^3 -> x^5
            } else if self.is_mul()
                && let Node::Binary(Binary::Pow(Pow { base, exp })) =
                    term.node()
                && let Node::Const(exp) = exp.node()
                && exp.value().is_integer()
            {
                if let Node::Variadic(Variadic::Mul(terms)) = base.node() {
                    for term in terms {
                        *groupings.entry(term.clone()).or_insert(0.0.into()) +=
                            exp.value();
                    }
                } else {
                    *groupings.entry(base.clone()).or_insert(0.0.into()) +=
                        exp.value();
                }
            /* -------------------------------------------------------------------------- */
            } else {
                *groupings.entry(term.clone()).or_insert(0.0.into()) += 1;
            }
        }

        // TODO: no need to allocate twice here, first in aggregated then in common if its add
        let mut aggregated: Vec<Expr> = match self {
            Variadic::Add(_) => groupings
                .into_iter()
                .map(|(base, coef)| {
                    if coef == 1.0.into() {
                        base
                    } else if coef == 0.0.into() {
                        0.0.into()
                    } else {
                        base * coef
                    }
                    .normalize(false)
                })
                .collect(),
            Variadic::Mul(_) => groupings
                .into_iter()
                .map(|(base, exp)| {
                    if exp == 1.0.into() {
                        base
                    } else if exp == 0.0.into() {
                        1.0.into()
                    } else {
                        pow(base, exp)
                    }
                    .normalize(false)
                })
                .collect(),
        };

        /* -------------------------------------------------------------------------- */
        // Partial factoring -> x * a + x * b -> x(a + b)
        // x * a + x * b + y * c + y * d -> x(a+b) + y(b+c)
        // TODO

        // if self.is_add() && aggregated.len() >= 2 {
        //     // TODO: when domain is implemented allow fractional exponents to be factored as well
        //     //
        //     // I really wrote this. With my free will.
        //     //
        //     // The factor table stores the individual factors for each term in this addition and their exponents,
        //     // as well as the term's coefficient.
        //     //
        //     // For the expression (x * y * 3) + (4 * x^2 / y) + (y^3) + (4y^2) + (8y^2 * x) it would look something like:
        //     // [
        //     //  ({ x: 1, y:  1 }, 3)
        //     //  ({ x: 2, y: -1 }, 4)
        //     //  ({       y: 3  }, 1)
        //     //  ({       y: 2  }, 4)
        //     //  ({ x: 1, y: 2  }, 8)
        //     // ]
        //     // We want to factor this into: y * ((x * 3) + y * (4 + 8x + y)) + (4 * x^2 / y)
        //     // During factoring, powers over multiplication are expanded.
        //     // Then, we group unique factors by exponent sign, ignoring terms where its 0, producing 2 groups for each factor like so:
        //     // x:
        //     // + [
        //     //  ({ x: 1, y:  1 }, 3)
        //     //  ({ x: 2, y: -1 }, 4)
        //     //  ({ x: 1, y: 2  }, 8)
        //     // ]
        //     // - []
        //     //
        //     // y:
        //     // + [
        //     //  ({ x: 1, y:  1 }, 3)
        //     //  ({ y: 3        }, 1)
        //     //  ({ y: 2        }, 4)
        //     //  ({ x: 1, y: 2  }, 8)
        //     // ]
        //     // - [
        //     //  ({ x: 2, y: -1 }, 4)
        //     // ]
        //     //
        //     // The group with the most terms is factored first, and its terms are removed from other groups. Positive and negative groups are factored separately
        //     // We start from the term with the highest exp, then move out.
        //     // First we calculate what exp each level will take by subtracting the sum of every previous exp, starting from the lowest exp:
        //     // y -> y^1
        //     // y^2 -> y^(2-1) -> y^1
        //     // y^3 -> y^(3 - (1 + 1)) -> y^1
        //     //
        //     // Then we factor from the inside out, appending each previous level to the current sum, and mapping exponents:
        //     // y^3 maps -> y
        //     // For n factors with the same exp, the GCD of the coefficient must be taken to pull it out:
        //     //
        //     //  ({ y: 2        }, 4)
        //     //  ({ x: 1, y: 2  }, 8)
        //     //  <prev> -> ({ y: 1  }, 1)
        //     // GCD = 1
        //     //
        //     // y^2 maps -> y
        //     // 4(y^2) + 8(y^2 * x) + <prev> -> y * (4 + 8x + <prev>) -> y * (4 + 8x + y)
        //     //
        //     // y maps -> y
        //     // y * x * 3 + <prev> -> (y * x * 3) + (y * (4 + 8x + y)) -> y(3x + y(4 + 8x + y))
        //     //
        //     // After the first step, we have
        //     // x:
        //     // + []
        //     // - []
        //     //
        //     // y:
        //     // + []
        //     // - [
        //     //  ({ x: 2, y: -1 }, 4)
        //     // ]
        //     //
        //     // This would then be repeated until all terms are disjoint, which they already are.
        //     // The last term is left unfactored.

        //     #[derive(PartialEq, Clone, Debug)]
        //     struct Term {
        //         factors: AHashMap<Expr, i64>,
        //         coef: f64,
        //     }

        //     impl Term {
        //         fn new(factors: AHashMap<Expr, i64>, coef: f64) -> Self {
        //             Self { factors, coef }
        //         }
        //     }

        //     impl PartialOrd for Term {
        //         fn partial_cmp(
        //             &self,
        //             other: &Self,
        //         ) -> Option<std::cmp::Ordering> {
        //             Some(self.coef.partial_cmp(&other.coef).unwrap().then_with(
        //                 || {
        //                     self.factors
        //                         .iter()
        //                         .partial_cmp(other.factors.iter())
        //                         .unwrap()
        //                 },
        //             ))
        //         }
        //     }

        //     let mut remaining = Vec::<Option<Term>>::new();
        //     let mut groups =
        //         AHashMap::<Expr, AHashMap<i64, Vec<usize>>>::with_capacity(
        //             aggregated.len(),
        //         );

        //     aggregated.sort_unstable();
        //     let (lone_const, terms) = extract_const(&aggregated);

        //     /* -------------------------------------------------------------------------- */
        //     for term in terms {
        //         let mut term_data =
        //             Term::new(AHashMap::with_capacity(aggregated.len()), 1.0);

        //         fn match_term(term: Expr, acc: i64, term_data: &mut Term) {
        //             match term.node() {
        //                 Node::Variadic(Variadic::Mul(t)) => {
        //                     let (coef, factors) = extract_const(&t);

        //                     let coef = coef.unwrap_or(1.0.into());

        //                     for fac in factors {
        //                         match_term(fac, acc, term_data);
        //                     }

        //                     if let Some(coef) = coef.value().as_real() {
        //                         term_data.coef = coef;
        //                     } else {
        //                         term_data.factors.insert(coef.into(), acc);
        //                     }
        //                 }
        //                 Node::Binary(Binary::Pow(Pow { base, exp }))
        //                     if let Some(exp) = exp
        //                         .node()
        //                         .as_const()
        //                         .and_then(|qty| qty.value().as_integer()) =>
        //                 {
        //                     match_term(base.clone(), acc * exp, term_data);
        //                 }
        //                 Node::Const(_) => unreachable!(),
        //                 _ => {
        //                     term_data.factors.insert(term, acc);
        //                 }
        //             }
        //         }

        //         match_term(term, 1, &mut term_data);

        //         let term_idx = remaining.len();

        //         for (fac, exp) in term_data.factors.clone() {
        //             groups
        //                 .entry(fac)
        //                 .or_insert_with(AHashMap::new)
        //                 .entry(exp)
        //                 .or_insert_with(Vec::new)
        //                 .push(term_idx);
        //         }

        //         remaining.push(Some(term_data));
        //     }

        //     /* -------------------------------------------------------------------------- */
        //     // TODO: refactor this shit

        //     if !groups.is_empty() {
        //         println!("groups: {:#?}", groups);

        //         println!("Remaining terms: {:#?}", remaining);

        //         let mut groups = groups
        //             .into_iter()
        //             .map(|group| (group.0, group.1.into_iter().collect_vec()))
        //             .collect_vec();

        //         groups.sort_unstable_by(|a, b| {
        //             a.1.iter()
        //                 .map(|x| x.1.len())
        //                 .sum::<usize>()
        //                 .cmp(&b.1.iter().map(|x| x.1.len()).sum::<usize>())
        //                 .then_with(|| {
        //                     a.1.iter()
        //                         .map(|x| {
        //                             x.1.iter().map(|i| remaining[*i].as_ref())
        //                         })
        //                         .flatten()
        //                         .partial_cmp(
        //                             b.1.iter()
        //                                 .map(|x| {
        //                                     x.1.iter()
        //                                         .map(|i| remaining[*i].as_ref())
        //                                 })
        //                                 .flatten(),
        //                         )
        //                         .unwrap()
        //                 })
        //         });

        //         let mut factored = Vec::new();

        //         while !groups.is_empty() {
        //             let (fac, mut levels) = groups.pop().unwrap();

        //             levels.sort_unstable_by_key(|(exp, ..)| *exp);

        //             let partition_point =
        //                 levels.partition_point(|(exp, ..)| *exp > 0);
        //             let (pos, neg) = levels.split_at_mut(partition_point);

        //             for part in [pos, neg] {
        //                 for i in 0..part.len() {
        //                     let (prev, current) = part.split_at_mut(i);
        //                     let level = &mut current[0];

        //                     let prev = prev.last().map(|x| x.0).unwrap_or(0);

        //                     level.0 = level.0 - prev;
        //                 }

        //                 let mut prev = Expr::from(0.0);

        //                 for (delta_exp, terms) in part.iter_mut().rev() {
        //                     println!("{}", delta_exp);
        //                     println!(
        //                         "Terms: {:#?}",
        //                         terms
        //                             .iter()
        //                             .map(|i| remaining[*i]
        //                                 .as_ref()
        //                                 .map(|t| t.factors.clone()))
        //                             .collect_vec()
        //                     );

        //                     let pulled_out = terms
        //                         .iter()
        //                         .filter_map(|i| remaining[*i].take())
        //                         .reduce(|mut a, b| {
        //                             a.factors.retain(|base, exp| {
        //                                 if *base == fac {
        //                                     *exp = *delta_exp;
        //                                     return true;
        //                                 }

        //                                 match b.factors.get(base) {
        //                                     Some(other_exp) => {
        //                                         *exp = (*exp).min(*other_exp);
        //                                         true
        //                                     }
        //                                     None => false,
        //                                 }
        //                             });

        //                             Term::new(
        //                                 a.factors,
        //                                 gcd_f64(a.coef, b.coef),
        //                             )
        //                         })
        //                         .or(terms
        //                             .iter()
        //                             .filter_map(|i| remaining[*i].take())
        //                             .map(|mut x| {
        //                                 *x.factors.get_mut(&fac).unwrap() =
        //                                     *delta_exp;
        //                                 x
        //                             })
        //                             .at_most_one()
        //                             .ok()
        //                             .flatten())
        //                         .unwrap_or(Term::new(AHashMap::new(), 1.0));

        //                     if !pulled_out.factors.is_empty()
        //                         || pulled_out.coef != 1.0
        //                     {
        //                         let common_factor =
        //                             (pulled_out.factors.iter().fold(
        //                                 Expr::from(1.0),
        //                                 |acc, (base, exp)| acc * (base ^ *exp),
        //                             ) * pulled_out.coef)
        //                                 .simplify_inner(ctx);

        //                         let mut sum = Vec::new();

        //                         sum.push(prev);

        //                         println!("Pulled out: {:#?}", pulled_out);
        //                         println!("Common factor: {}", common_factor);
        //                         prev = common_factor
        //                             * Expr::from(Variadic::Add(sum))
        //                     }
        //                 }
        //                 factored.push(prev);
        //             }
        //         }

        //         aggregated = factored;
        //         if let Some(qty) = lone_const {
        //             aggregated.push(qty.into());
        //         }
        //     }
        // }

        /* -------------------------------------------------------------------------- */

        if aggregated.len() <= 1 {
            aggregated.pop().unwrap_or(0.into())
        } else {
            self.with_operands(aggregated).into()
        }
    }
}

pub fn separate_consts(
    terms: impl Iterator<Item = Expr> + Clone,
) -> (impl Iterator<Item = Quantity>, impl Iterator<Item = Expr>) {
    (
        terms.clone().into_iter().filter_map(|expr| match expr.node() {
            Node::Const(qty) => Some(*qty),
            _ => None,
        }),
        terms.into_iter().filter(|expr| !expr.node().is_const()),
    )
}

pub fn extract_const(
    terms: &Vec<Expr>,
) -> (Option<Quantity>, impl Iterator<Item = Expr>) {
    let constant =
        terms.get(0).and_then(|x| x.clone().into_node().as_const().copied());

    let exprs = terms.iter().cloned().filter(|expr| !expr.node().is_const());

    (constant, exprs)
}
