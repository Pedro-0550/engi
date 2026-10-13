use std::collections::HashMap;

use num::{Zero, complex::Complex64};

use crate::{
    core::value::{EQ_ABS_TOL, Set},
    expr::{
        self, Expr, Operand, cos, cosh,
        dag::{Leaf, Operation},
        domain::{Domain, Numeric},
        ln, sin, sinh, sqrt,
    },
    symbol::{Symbol, constants::e},
};

/* --------------------------------- MODULES -------------------------------- */

#[cfg(test)]
mod test;

/* --------------------------------- TRAITS --------------------------------- */

// enum Derivative {
//     Conventional { dz: Expr },
//     Wirtinger { dz: Expr, dz_conj: Expr },
// }
/* ---------------------------------- IMPLS --------------------------------- */

impl Expr {
    /// Takes the partial derivative of this expression, with respect to the given symbol
    /// The derivative is simplified before being returned.
    pub fn diff(&self, wrt: Symbol) -> Expr {
        let mut dx = Expr::new();

        let (_, du) = self.fold(|op, d_op| {
            (
                op,
                match d_op {
                    Operand::Symbol(symbol) => {
                        let ctx = dx.edit();

                        if wrt == symbol {
                            ctx.one()
                        } else {
                            ctx.zero()
                        }
                    }
                    Operand::Leaf(leaf) => {
                        let ctx = dx.edit();
                        match leaf {
                            Leaf::Constant(_) => ctx.zero(),
                            Leaf::Quantity(_) => ctx.zero(),
                        }
                    }
                    Operand::Operation(operation) => match operation {
                        Operation::Add([(_, da), (_, db)]) => {
                            let ctx = dx.edit();
                            ctx.add(*da, *db)
                        }
                        Operation::Mul([(a, da), (b, db)]) => {
                            let a = dx.import(self, *a);
                            let b = dx.import(self, *b);
                            let ctx = dx.edit();
                            ctx.add(ctx.mul(a, *db), ctx.mul(*da, b))
                        }
                        Operation::Min(_) => todo!(),
                        Operation::Max(_) => todo!(),

                        Operation::Sin((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.mul(*du, ctx.cos(u))
                        }
                        Operation::Cos((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.mul(*du, ctx.neg(ctx.sin(u)))
                        }
                        Operation::Tan((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(*du, ctx.pow(ctx.cos(u), ctx.qty(2)))
                        }

                        Operation::Asin((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(
                                *du,
                                ctx.sqrt(
                                    ctx.sub(ctx.one(), ctx.pow(u, ctx.qty(2))),
                                ),
                            )
                        }
                        Operation::Acos((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(
                                ctx.neg(*du),
                                ctx.sqrt(
                                    ctx.sub(ctx.one(), ctx.pow(u, ctx.qty(2))),
                                ),
                            )
                        }
                        Operation::Atan((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(
                                *du,
                                ctx.add(ctx.pow(u, ctx.qty(2)), ctx.one()),
                            )
                        }

                        Operation::Sinh((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.mul(*du, ctx.cosh(u))
                        }
                        Operation::Cosh((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.mul(*du, ctx.sinh(u))
                        }
                        Operation::Tanh((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(*du, ctx.pow(ctx.cosh(u), ctx.qty(2)))
                        }

                        Operation::Asinh((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(
                                *du,
                                ctx.sqrt(
                                    ctx.add(ctx.pow(u, ctx.qty(2)), ctx.one()),
                                ),
                            )
                        }
                        Operation::Acosh((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(
                                *du,
                                ctx.sqrt(
                                    ctx.sub(ctx.pow(u, ctx.qty(2)), ctx.one()),
                                ),
                            )
                        }
                        Operation::Atanh((u, du)) => {
                            let u = dx.import(self, *u);
                            let ctx = dx.edit();
                            ctx.div(
                                *du,
                                ctx.sub(ctx.one(), ctx.pow(u, ctx.qty(2))),
                            )
                        }

                        Operation::Arg(_) => todo!(),
                        Operation::Conj(_) => todo!(),
                        Operation::Norm(_) => todo!(),
                        Operation::Sign(_) => dx.edit().zero(),
                        Operation::Real((u, du)) => match self.domain_of(*u).numeric() {
                            Numeric::Real => *du,
                            Numeric::Imag => dx.edit().zero(),
                            Numeric::Complex => unimplemented!("Cannot take derivative of Re{{z}} when z is a complex number"),
                        },

                        Operation::Imag((u, du)) => match self.domain_of(*u).numeric() {
                            Numeric::Imag => *du,
                            Numeric::Real => dx.edit().zero(),
                            Numeric::Complex => unimplemented!("Cannot take derivative of Re{{z}} when z is a complex number"),
                        },


                        Operation::Pow {
                            base: (base, d_base),
                            exp: (exp, d_exp),
                        } => {
                            let base = dx.import(self, *base);
                            let exp = dx.import(self, *exp);
                            let ctx = dx.edit();
                            if *d_exp == ctx.zero() {
                                ctx.mul(
                                    exp,
                                    ctx.mul(
                                        ctx.pow(base, ctx.sub(exp, ctx.qty(1))),
                                        *d_base,
                                    ),
                                )
                            } else {
                                ctx.mul(
                                    ctx.pow(base, exp),
                                    ctx.add(
                                        ctx.div(ctx.mul(*d_base, exp), base),
                                        ctx.mul(*d_exp, ctx.ln(base)),
                                    ),
                                )
                            }
                        }

                        Operation::Log {
                            base: (base, d_base),
                            arg: (arg, d_arg),
                        } => {
                            let base = dx.import(self, *base);
                            let arg = dx.import(self, *arg);
                            let ctx = dx.edit();
                            if *d_base == ctx.zero() {
                                ctx.div(*d_arg, ctx.mul(arg, ctx.ln(base)))
                            } else {
                                ctx.div(
                                    ctx.sub(
                                        ctx.mul(
                                            ctx.div(*d_arg, arg),
                                            ctx.ln(base),
                                        ),
                                        ctx.mul(
                                            ctx.div(*d_base, base),
                                            ctx.ln(arg),
                                        ),
                                    ),
                                    ctx.pow(ctx.ln(base), ctx.qty(2)),
                                )
                            }
                        }

                        Operation::Atan2 { x: (a, da), y: (b, db) } => {
                            let a = dx.import(self, *a);
                            let b = dx.import(self, *b);
                            let ctx = dx.edit();
                            ctx.div(
                                ctx.sub(ctx.mul(b, *da), ctx.mul(a, *db)),
                                ctx.add(
                                    ctx.pow(a, ctx.qty(2)),
                                    ctx.pow(b, ctx.qty(2)),
                                ),
                            )
                        }

                        Operation::Transpose(_) => todo!(),
                        Operation::Det(_) => todo!(),
                        Operation::Rank(_) => todo!(),
                        Operation::Trace(_) => todo!(),
                        Operation::Conditional { cond, pass: (pass, d_pass), fail: (_, d_fail) } => {
                            todo!()
                        }
                    },
                    Operand::Matrix(m) => dx.push(Operand::Matrix(m.map(|(el, d_el)| *d_el))),
                    Operand::Wildcard(never) => never,
                },
            )
        });

        dx.set_root(du);
        dx.simplified()
    }
}
