use std::collections::HashMap;

use num::{Zero, complex::Complex64};

use crate::{
    core::value::{EQ_ABS_TOL, Set},
    expr::{
        self, Expr, NodeId, cos, cosh,
        domain::{Domain, Numeric},
        ln, sin, sinh, sqrt,
        tree::{Branch, Leaf, Node},
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
    pub fn diff(&self, s: Symbol) -> Expr {
        let mut dx = Expr::new();

        let (u, du) = self.fold(|id, _, node| {
            (
                id,
                match node {
                    Node::Leaf(leaf) => {
                        let ctx = dx.edit();
                        match leaf {
                            Leaf::Symbol(symbol) => {
                                if s == symbol {
                                    ctx.one()
                                } else {
                                    ctx.zero()
                                }
                            }
                            Leaf::Constant(_) => ctx.zero(),
                            Leaf::Quantity(_) => ctx.zero(),
                        }
                    }
                    Node::Branch(branch) => match branch {
                        Branch::Add([(_, da), (_, db)]) => {
                            let ctx = dx.edit();
                            ctx.add(da, db)
                        }
                        Branch::Mul([(a, da), (b, db)]) => {
                            let a = dx.import(self, a);
                            let b = dx.import(self, b);
                            let ctx = dx.edit();
                            ctx.add(ctx.mul(a, db), ctx.mul(da, b))
                        }
                        Branch::Min(_) => todo!(),
                        Branch::Max(_) => todo!(),

                        Branch::Sin((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.mul(du, ctx.cos(u))
                        }
                        Branch::Cos((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.mul(du, ctx.neg(ctx.sin(u)))
                        }
                        Branch::Tan((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(du, ctx.pow(ctx.cos(u), ctx.qty(2)))
                        }

                        Branch::Asin((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(
                                du,
                                ctx.sqrt(
                                    ctx.sub(ctx.one(), ctx.pow(u, ctx.qty(2))),
                                ),
                            )
                        }
                        Branch::Acos((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(
                                ctx.neg(du),
                                ctx.sqrt(
                                    ctx.sub(ctx.one(), ctx.pow(u, ctx.qty(2))),
                                ),
                            )
                        }
                        Branch::Atan((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(
                                du,
                                ctx.add(ctx.pow(u, ctx.qty(2)), ctx.one()),
                            )
                        }

                        Branch::Sinh((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.mul(du, ctx.cosh(u))
                        }
                        Branch::Cosh((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.mul(du, ctx.sinh(u))
                        }
                        Branch::Tanh((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(du, ctx.pow(ctx.cosh(u), ctx.qty(2)))
                        }

                        Branch::Asinh((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(
                                du,
                                ctx.sqrt(
                                    ctx.add(ctx.pow(u, ctx.qty(2)), ctx.one()),
                                ),
                            )
                        }
                        Branch::Acosh((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(
                                du,
                                ctx.sqrt(
                                    ctx.sub(ctx.pow(u, ctx.qty(2)), ctx.one()),
                                ),
                            )
                        }
                        Branch::Atanh((u, du)) => {
                            let u = dx.import(self, u);
                            let ctx = dx.edit();
                            ctx.div(
                                du,
                                ctx.sub(ctx.one(), ctx.pow(u, ctx.qty(2))),
                            )
                        }

                        Branch::Arg(_) => todo!(),
                        Branch::Conj(_) => todo!(),
                        Branch::Norm(_) => todo!(),
                        Branch::Sign(_) => dx.edit().zero(),
                        Branch::Real((u, du)) => match self.domain_of(u).numeric() {
                            Numeric::Real => du,
                            Numeric::Imag => dx.edit().zero(),
                            Numeric::Complex => unimplemented!("Cannot take derivative of Re{{z}} when z is a complex number"),
                        },

                        Branch::Imag((u, du)) => match self.domain_of(u).numeric() {
                            Numeric::Imag => du,
                            Numeric::Real => dx.edit().zero(),
                            Numeric::Complex => unimplemented!("Cannot take derivative of Re{{z}} when z is a complex number"),
                        },


                        Branch::Pow {
                            base: (base, d_base),
                            exp: (exp, d_exp),
                        } => {
                            let base = dx.import(self, base);
                            let exp = dx.import(self, exp);
                            let ctx = dx.edit();
                            if d_exp == ctx.zero() {
                                ctx.mul(
                                    exp,
                                    ctx.mul(
                                        ctx.pow(base, ctx.sub(exp, ctx.qty(1))),
                                        d_base,
                                    ),
                                )
                            } else {
                                ctx.mul(
                                    ctx.pow(base, exp),
                                    ctx.add(
                                        ctx.div(ctx.mul(d_base, exp), base),
                                        ctx.mul(d_exp, ctx.ln(base)),
                                    ),
                                )
                            }
                        }

                        Branch::Log {
                            base: (base, d_base),
                            arg: (arg, d_arg),
                        } => {
                            let base = dx.import(self, base);
                            let arg = dx.import(self, arg);
                            let ctx = dx.edit();
                            if d_base == ctx.zero() {
                                ctx.div(d_arg, ctx.mul(arg, ctx.ln(base)))
                            } else {
                                ctx.div(
                                    ctx.sub(
                                        ctx.mul(
                                            ctx.div(d_arg, arg),
                                            ctx.ln(base),
                                        ),
                                        ctx.mul(
                                            ctx.div(d_base, base),
                                            ctx.ln(arg),
                                        ),
                                    ),
                                    ctx.pow(ctx.ln(base), ctx.qty(2)),
                                )
                            }
                        }

                        Branch::Atan2 { a: (a, da), b: (b, db) } => {
                            let a = dx.import(self, a);
                            let b = dx.import(self, b);
                            let ctx = dx.edit();
                            ctx.div(
                                ctx.sub(ctx.mul(b, da), ctx.mul(a, db)),
                                ctx.add(
                                    ctx.pow(a, ctx.qty(2)),
                                    ctx.pow(b, ctx.qty(2)),
                                ),
                            )
                        }

                        Branch::Matrix(_) => todo!(),
                        Branch::Transpose(_) => todo!(),
                        Branch::Det(_) => todo!(),
                        Branch::Rank(_) => todo!(),
                        Branch::Trace(_) => todo!(),
                        Branch::Conditional { cond, pass: (pass, d_pass), fail: (_, d_fail) } => {
                            todo!()
                        }
                    },
                },
            )
        });

        dx.set_root(du);
        // println!("non simp: {}", dx.len());
        let simp = dx.simplified();
        // println!("simp: {}", simp.len());

        simp
    }
}
