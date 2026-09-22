/* --------------------------------- STRUCTS -------------------------------- */

use std::mem;

use ahash::HashMap;
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

use crate::{
    core::value,
    expr::{Expr, Node, domain::Domain},
    normal::Normalize,
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

        let Complex { re: re_expr, im: im_expr } = self.clone().realize();

        println!("Compiling ({}) + i * ({})", re_expr, im_expr);

        let mut args = [re_expr.symbols(), im_expr.symbols()].concat();
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
            let compiled_children = expr
                .iter_children()
                .map(|x| compile_realized_expr(x, symbols, bcx, fns))
                .collect_vec();

            match &expr.node {
                Node::Symbol(symbol) => symbols.get(symbol).copied().unwrap(),
                Node::Constant(constant) => {
                    let qty = constant
                        .quantity()
                        .value()
                        .clone()
                        .into_scalar()
                        .unwrap();

                    bcx.ins().f64const(qty.re)
                }
                Node::Quantity(quantity) => {
                    let qty =
                        quantity.clone().value().clone().into_scalar().unwrap();
                    bcx.ins().f64const(qty.re)
                }
                Node::Add(exprs) => compiled_children
                    .into_iter()
                    .reduce(|a, b| bcx.ins().fadd(a, b))
                    .unwrap(),
                Node::Mul(exprs) => compiled_children
                    .into_iter()
                    .reduce(|a, b| bcx.ins().fmul(a, b))
                    .unwrap(),

                Node::Min(exprs) => compiled_children
                    .into_iter()
                    .reduce(|a, b| bcx.ins().fmin(a, b))
                    .unwrap(),
                Node::Max(exprs) => compiled_children
                    .into_iter()
                    .reduce(|a, b| bcx.ins().fmax(a, b))
                    .unwrap(),

                Node::Sin(expr) => fns.sin(bcx, compiled_children[0]),
                Node::Cos(expr) => fns.cos(bcx, compiled_children[0]),
                Node::Tan(expr) => fns.tan(bcx, compiled_children[0]),
                Node::Asin(expr) => fns.asin(bcx, compiled_children[0]),
                Node::Acos(expr) => fns.acos(bcx, compiled_children[0]),
                Node::Atan(expr) => fns.atan(bcx, compiled_children[0]),
                Node::Sinh(expr) => fns.sinh(bcx, compiled_children[0]),
                Node::Cosh(expr) => fns.cosh(bcx, compiled_children[0]),
                Node::Tanh(expr) => fns.tanh(bcx, compiled_children[0]),
                Node::Asinh(expr) => fns.asinh(bcx, compiled_children[0]),
                Node::Acosh(expr) => fns.acosh(bcx, compiled_children[0]),
                Node::Atanh(expr) => fns.atanh(bcx, compiled_children[0]),
                Node::Transpose(expr) => todo!(),
                Node::Conj(expr) => todo!(),
                Node::Arg(expr) => todo!(),
                Node::Det(expr) => todo!(),
                Node::Norm(expr) => fns.abs(bcx, compiled_children[0]),
                Node::Real(expr) => compiled_children[0],
                Node::Sign(expr) => fns.signum(bcx, compiled_children[0]),
                Node::Imag(expr) => bcx.ins().f64const(0.0),

                Node::Pow { base, .. } => {
                    if **base == e {
                        fns.exp(bcx, compiled_children[1])
                    } else {
                        fns.powf(
                            bcx,
                            compiled_children[0],
                            compiled_children[1],
                        )
                    }
                }
                Node::Log { base, .. } => {
                    if **base == e {
                        fns.ln(bcx, compiled_children[1])
                    } else {
                        let num = fns.ln(bcx, compiled_children[1]);
                        let denom = fns.ln(bcx, compiled_children[0]);
                        bcx.ins().fdiv(num, denom)
                    }
                }
                Node::Atan2 { .. } => {
                    fns.atan2(bcx, compiled_children[0], compiled_children[1])
                }

                Node::Matrix(matrix) => todo!(),
                Node::Rank(expr) => todo!(),
                Node::Trace(expr) => todo!(),
                Node::Piecewise { cond, pass, fail } => {
                    todo!()
                }
            }
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
