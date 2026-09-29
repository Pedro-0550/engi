/* --------------------------------- STRUCTS -------------------------------- */

use std::{collections::HashMap, mem};

use cranelift::{
    codegen::{
        ir::{
            AbiParam, ExtFuncData, ExternalName, FuncRef, InstBuilder,
            MemFlagsData, UserFuncName,
            types::{self, F64},
        },
        settings::{self, Configurable},
    },
    frontend::{FunctionBuilder, FunctionBuilderContext},
    jit::{JITBuilder, JITModule},
    module::{Linkage, Module, default_libcall_names},
    prelude::{self, Value},
};
use itertools::Itertools;
use num::{Complex, complex::Complex64};
use xxhash_rust::xxh3::{Xxh3, Xxh3Builder};

use crate::{
    core::value,
    expr::{
        Expr, Node,
        domain::Domain,
        tree::{Branch, Leaf},
    },
    symbol::{
        Realization::{self, Imag, Primary, Real},
        Symbol,
        constants::e,
    },
};

pub struct CompiledExpr {
    func: unsafe extern "C" fn(*const f64, *mut f64),
    args: Vec<Symbol>,
    module: JITModule,
}

/* ---------------------------------- IMPLS --------------------------------- */

macro_rules! impl_math_fns {
    ($($unary:ident),*; $($binary:ident),*) => {
        struct MathFns {
            $($unary: FuncRef,)*
            $($binary: FuncRef,)*
        }


            $(unsafe extern "C" fn $unary(x: f64) -> f64 {
                x.$unary()
            })*
            $(unsafe extern "C" fn $binary(a: f64, b: f64) -> f64 {
                a.$binary(b)
            })*


        impl MathFns{
            $(
                fn $unary(&self, bcx: &mut FunctionBuilder<'_>, val: Value) -> Value {
                    let call = bcx.ins().call(self.$unary, &[val]);
                    bcx.inst_results(call)[0]
                }
            )*

            $(
                fn $binary(&self, bcx: &mut FunctionBuilder<'_>, a: Value, b: Value) -> Value {
                    let call = bcx.ins().call(self.$binary, &[a, b]);
                    bcx.inst_results(call)[0]
                }
            )*


            fn add_symbols(builder: &mut JITBuilder) {
                $(builder.symbol(concat!("engi_", stringify!($unary)), $unary as *const u8);)*
                $(builder.symbol(concat!("engi_", stringify!($binary)), $binary as *const u8);)*
            }

            fn new(module: &mut JITModule, bcx: &mut FunctionBuilder<'_>) -> Self {
                let default_call_conv = module.target_config().default_call_conv;

                let mut unary_sig = module.make_signature();
                unary_sig.call_conv = default_call_conv;
                unary_sig.params.push(AbiParam::new(F64));
                unary_sig.returns.push(AbiParam::new(F64));

                let mut binary_sig = module.make_signature();
                binary_sig.call_conv = default_call_conv;
                binary_sig.params.push(AbiParam::new(F64));
                binary_sig.params.push(AbiParam::new(F64));
                binary_sig.returns.push(AbiParam::new(F64));

                Self {
                    $(
                    $unary: {
                        let $unary = module
                            .declare_function(concat!("engi_", stringify!($unary)), Linkage::Import, &unary_sig)
                            .unwrap();
                        module.declare_func_in_func($unary, bcx.func)
                    },
                    )*
                    $(
                    $binary: {
                        let $binary = module
                            .declare_function(concat!("engi_", stringify!($binary)), Linkage::Import, &binary_sig)
                            .unwrap();
                        module.declare_func_in_func($binary, bcx.func)
                    },
                    )*
                }
            }
        }
    };
}

impl_math_fns!(sin, cos, tan, asin, acos, atan, sinh, cosh, tanh, asinh, acosh, atanh, exp, ln, sqrt, abs, signum; powf, atan2);

impl Expr {
    pub fn compile(&self) -> CompiledExpr {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed_and_size").unwrap();
        flag_builder.set("enable_verifier", "true").unwrap();

        let isa_builder = cranelift::native::builder().unwrap_or_else(|msg| {
            panic!("host machine is not supported: {msg}");
        });

        let isa =
            isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
        let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
        MathFns::add_symbols(&mut builder);

        let mut module = JITModule::new(builder);

        let mut ctx = module.make_context();
        let mut func_ctx = FunctionBuilderContext::new();

        let mut eval_sig = module.make_signature();
        eval_sig.call_conv = module.target_config().default_call_conv;
        eval_sig
            .params
            .push(AbiParam::new(module.target_config().pointer_type()));
        eval_sig
            .params
            .push(AbiParam::new(module.target_config().pointer_type())); // output

        let eval_func =
            module.declare_function("eval", Linkage::Local, &eval_sig).unwrap();

        ctx.func.signature = eval_sig;
        ctx.func.name = UserFuncName::user(0, eval_func.as_u32());

        println!("Compiling base expr: {}", self);

        let [re_expr, im_expr] = self.clone().realize();

        println!("Compiling ({}) + i * ({})", re_expr, im_expr);

        let mut args = re_expr.symbols().chain(im_expr.symbols()).collect_vec();
        args.sort();
        args.dedup();

        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = bcx.create_block();

        bcx.switch_to_block(block);
        bcx.append_block_params_for_function_params(block);

        let fns = MathFns::new(&mut module, &mut bcx);

        let symbols_ptr = bcx.block_params(block)[0];
        let output_ptr = bcx.block_params(block)[1];
        let symbols: HashMap<_, _> = args
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    *s,
                    bcx.ins().load(
                        F64,
                        MemFlagsData::new().with_aligned(),
                        symbols_ptr,
                        (i * 8) as i32,
                    ),
                )
            })
            .collect();

        fn compile_realized_expr(
            expr: &Expr,
            symbols: &HashMap<Symbol, Value>,
            bcx: &mut FunctionBuilder<'_>,
            fns: &MathFns,
        ) -> Value {
            expr.clone().fold_dfs(|_, node| match node {
                Node::Leaf(leaf) => match leaf {
                    Leaf::Symbol(symbol) => {
                        symbols.get(symbol).copied().unwrap()
                    }
                    Leaf::Constant(constant) => {
                        let qty = constant
                            .quantity()
                            .value()
                            .clone()
                            .into_scalar()
                            .unwrap();

                        bcx.ins().f64const(qty.re)
                    }
                    Leaf::Quantity(quantity) => {
                        let qty = quantity
                            .clone()
                            .value()
                            .clone()
                            .into_scalar()
                            .unwrap();
                        bcx.ins().f64const(qty.re)
                    }
                },
                Node::Branch(branch) => match branch {
                    Branch::Add([a, b]) => bcx.ins().fadd(*a, *b),
                    Branch::Mul([a, b]) => bcx.ins().fmul(*a, *b),
                    Branch::Min([a, b]) => bcx.ins().fmin(*a, *b),
                    Branch::Max([a, b]) => bcx.ins().fmax(*a, *b),
                    Branch::Sin(v) => fns.sin(bcx, *v),
                    Branch::Cos(v) => fns.cos(bcx, *v),
                    Branch::Tan(v) => fns.tan(bcx, *v),
                    Branch::Asin(v) => fns.asin(bcx, *v),
                    Branch::Acos(v) => fns.acos(bcx, *v),
                    Branch::Atan(v) => fns.atan(bcx, *v),
                    Branch::Sinh(v) => fns.sinh(bcx, *v),
                    Branch::Cosh(v) => fns.cosh(bcx, *v),
                    Branch::Tanh(v) => fns.tanh(bcx, *v),
                    Branch::Asinh(v) => fns.asinh(bcx, *v),
                    Branch::Acosh(v) => fns.acosh(bcx, *v),
                    Branch::Atanh(v) => fns.atanh(bcx, *v),
                    Branch::Arg(_) => todo!(),
                    Branch::Conj(v) => *v,
                    Branch::Norm(v) => fns.abs(bcx, *v),
                    Branch::Sign(v) => fns.signum(bcx, *v),
                    Branch::Real(v) => *v,
                    Branch::Imag(_) => bcx.ins().f64const(0.0),
                    Branch::Pow { base, exp } => fns.powf(bcx, *base, *exp),
                    Branch::Log { base, arg } => {
                        let a = fns.ln(bcx, *arg);
                        let b = fns.ln(bcx, *base);

                        bcx.ins().fdiv(a, b)
                    }
                    Branch::Atan2 { a, b } => fns.atan2(bcx, *a, *b),
                    Branch::Matrix(matrix) => todo!(),
                    Branch::Transpose(_) => todo!(),
                    Branch::Det(_) => todo!(),
                    Branch::Rank(_) => todo!(),
                    Branch::Trace(_) => todo!(),
                    Branch::Conditional { cond, pass, fail } => {}
                },
            })
        }

        let (re, im) = (
            compile_realized_expr(&re_expr, &symbols, &mut bcx, &fns),
            compile_realized_expr(&im_expr, &symbols, &mut bcx, &fns),
        );

        bcx.ins().store(MemFlagsData::trusted(), re, output_ptr, 0);
        bcx.ins().store(MemFlagsData::trusted(), im, output_ptr, 8);

        bcx.ins().return_(&[]);
        bcx.seal_all_blocks();
        bcx.finalize(module.target_config());

        module.define_function(eval_func, &mut ctx).unwrap();
        println!("--- CRANELIFT IR ---\n{}", ctx.func.display());
        module.clear_context(&mut ctx);
        module.finalize_definitions().unwrap();

        let eval_ptr = unsafe {
            mem::transmute::<_, extern "C" fn(*const f64, *mut f64)>(
                module.get_finalized_function(eval_func),
            )
        };

        CompiledExpr { args, func: eval_ptr, module }
    }
}

impl CompiledExpr {
    pub fn eval(&self, bindings: &HashMap<Symbol, Complex64>) -> value::Value {
        let assembled_args = self
            .args
            .iter()
            .map(|s| {
                let c = bindings.get(&s.as_primary().unwrap()).unwrap();
                match s.realization() {
                    Realization::Real(_) => c.re,
                    Realization::Imag(_) => c.im,
                    Realization::Primary => unreachable!(),
                }
            })
            .collect_vec();

        let mut output = [0.0_f64; 2];

        unsafe {
            (self.func)(assembled_args.as_ptr(), output.as_mut_ptr());
        }

        Complex64::new(output[0], output[1]).into()
    }

    pub fn eval_realized(
        &self,
        bindings: &HashMap<Symbol, f64>,
    ) -> value::Value {
        let assembled_args =
            self.args.iter().map(|s| *bindings.get(s).unwrap()).collect_vec();

        let mut output = [0.0_f64; 2];

        unsafe {
            (self.func)(assembled_args.as_ptr(), output.as_mut_ptr());
        }

        Complex64::new(output[0], output[1]).into()
    }
}
