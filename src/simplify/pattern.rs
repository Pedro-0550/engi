/* --------------------------------- STRUCTS -------------------------------- */

use std::{collections::HashMap, mem, ops::Neg, rc::Rc};

use ahash::AHashMap;
use derive_more::Into;
// use cranelift::codegen::ir::Constant;
use kinded::Kinded;
use num::{complex::Complex64, pow::Pow};

use crate::{
    core::{index::Indexed, util::impl_op_permutations, value::Value},
    expr::{
        Expr, Operand,
        dag::{self, Dag, LeafId, MatrixId, OperationId, OperationKind},
        domain::Domain,
        shape::Shape,
    },
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    simplify::{
        ClassId, EquivalencyClass, EquivalencyGraph, Leaf, Substitution,
    },
    symbol::{
        Symbol,
        constants::{Constant, e},
    },
    units::{Quantity, Unit},
};

type Allocations = Indexed<[Option<RegisterId>; WILDCARD_COUNT], Wildcard>;
type Registers = Indexed<Vec<ClassId>, RegisterId>;

// This should be updated if we ever use more than 4 wildcard ids in rules
pub const WILDCARD_COUNT: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Into)]
pub struct RegisterId(u8);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Into)]
pub struct Wildcard(pub(crate) u8);

pub struct Program {
    code: Vec<Instruction>,
    allocations: Allocations,
    register_count: u8,
}

pub struct Machine;

#[derive(Clone)]
pub struct Condition {
    pub over: Box<[Wildcard]>,
    pub f: fn(ConditionContext<'_>) -> bool,
}

#[derive(Clone)]
pub struct Rule {
    pub from: Pattern,
    pub to: Pattern,
    pub conds: Box<[Condition]>,
}

struct CompilerState {
    instructions: Vec<Instruction>,
    next: RegisterId,
    allocations: Allocations,
}

pub struct ConditionContext<'a> {
    registers: &'a Registers,
    allocations: &'a Allocations,
    graph: &'a EquivalencyGraph,
}

pub enum Instruction {
    Bind { structure: OperandStructure, target: RegisterId, out: RegisterId },
    Compare { a: RegisterId, b: RegisterId },
    If { f: fn(ConditionContext<'_>) -> bool },
}

pub type Pattern = Expr<Wildcard>;

/// Encapusalates all information required to check if two operands can be bound:
/// * Leaves need to be equal, and since leaves are interned we can just compile tailored for each expr and compare IDs
/// * Matrices only need to be of the same shape
/// * Operations only need to be of the same kind
pub enum OperandStructure {
    Leaf(LeafId),
    Matrix(Shape),
    Operation(OperationKind),
}

/* -------------------------------------------------------------------------- */

impl<'a> ConditionContext<'a> {
    pub fn class(&self, wildcard: Wildcard) -> Option<&'a EquivalencyClass> {
        let reg = self.allocations[wildcard]?;
        let class_id = self.registers[reg];

        let root_id = self.graph.find(class_id);
        Some(&self.graph.classes[root_id])
    }

    pub fn domain(&self, wildcard: Wildcard) -> Domain {
        self.class(wildcard).unwrap().domain()
    }

    pub fn shape(&self, wildcard: Wildcard) -> Shape {
        self.class(wildcard).unwrap().shape()
    }
}

impl Pattern {
    pub(crate) fn compile(&self, conditions: &[Condition]) -> Program {
        let mut state = CompilerState {
            instructions: Vec::new(),
            next: RegisterId(1),
            allocations: [None; WILDCARD_COUNT].into(),
        };

        Self::compile_inner(
            self,
            RegisterId(0),
            &mut conditions.iter().map(Option::Some).collect::<Box<[_]>>(),
            &mut state,
        );

        Program {
            code: state.instructions,
            allocations: state.allocations,
            register_count: state.next.0,
        }
    }

    fn compile_inner(
        &self,
        target: RegisterId,
        conditions: &mut [Option<&Condition>],
        state: &mut CompilerState,
    ) {
        match self {
            Pattern::Wildcard(w) => {
                if let Some(&allocated) = state.allocations.get(w) {
                    state
                        .instructions
                        .push(Instruction::Compare { a: allocated, b: target });
                } else {
                    state.allocations.insert(*w, target);
                }

                for cd in conditions {
                    if let Some(cond) = cd.take_if(|cond| {
                        cond.over
                            .iter()
                            .all(|x| state.allocations.contains_key(x))
                    }) {
                        state
                            .instructions
                            .push(Instruction::If { f: cond.f.clone() });
                    }
                }
            }
            Pattern::Node(node) => match node {
                Node::Leaf(leaf) => match leaf {
                    _ => state.instructions.push(Instruction::Bind {
                        structure: EquivalencyNodeStructure::Leaf(leaf.clone()),
                        target,
                        out: state.next,
                    }),
                },
                Node::Branch(branch) => {
                    let out = state.next;
                    let children = node.children();
                    state.next.0 += node.children().len();

                    state.instructions.push(Instruction::Bind {
                        structure: EquivalencyNodeStructure::Branch(
                            branch.kind(),
                        ),
                        target,
                        out,
                    });

                    for (i, child) in children.iter().enumerate() {
                        child.compile_inner(
                            RegisterId(out.0 + i),
                            conditions,
                            state,
                        );
                    }
                }
            },
        }
    }
}

impl Machine {
    pub(crate) fn execute(
        graph: &EquivalencyGraph,
        program: &Program,
        root: ClassId,
        out: &mut Vec<Substitution>,
    ) {
        let mut registers = vec![root; program.register_count as usize].into();

        fn execute_inner(
            pc: usize,
            registers: &mut Registers,
            graph: &EquivalencyGraph,
            program: &Program,
            out: &mut Vec<Substitution>,
        ) {
            if pc >= program.code.len() {
                out.push(Substitution {
                    bindings: program
                        .allocations
                        .iter()
                        .filter_map(|(wildcard, reg)| {
                            Some((wildcard, graph.find(registers[(*reg)?])))
                        })
                        .collect(),
                });
                return;
            }

            match &program.code[pc] {
                Instruction::Bind { structure, target, out: bind_out } => {
                    let target_id = registers[*target];
                    let target_class = &graph.classes[graph.find(target_id)];
                    let exprs =
                        &target_class.exprs_by_kind[structure.kind().id()];

                    for expr_id in exprs {
                        let expr = &graph.exprs[expr_id.0];

                        if expr.node.structure() == *structure {
                            let mut i = 0;
                            expr.node.for_each_child(|child_id| {
                                registers[bind_out.0 + i] = *child_id;

                                i += 1;
                            });

                            execute_inner(
                                pc + 1,
                                registers,
                                graph,
                                program,
                                out,
                            );
                        }
                    }
                }
                Instruction::Compare { a, b } => {
                    let a_id = registers[*a];
                    let b_id = registers[*b];

                    if graph.find(a_id) == graph.find(b_id) {
                        execute_inner(pc + 1, registers, graph, program, out);
                    }
                }
                Instruction::If { f } => {
                    let ctx = ConditionContext {
                        allocations: &program.allocations,
                        graph,
                        registers: (&*registers).into(),
                    };

                    if f(ctx) {
                        execute_inner(pc + 1, registers, graph, program, out);
                    }
                }
            }
        }

        execute_inner(0, &mut registers, graph, program, out);
    }
}

impl_op_permutations!(
    types = [
        i64, f64, Complex64, &Complex64, Quantity, &Quantity, Value, &Value,
        Constant, &Constant, Symbol, &Symbol, Pattern, &Pattern, Wildcard,
        &Wildcard, Expr, &Expr
    ],
    exclude_permutations = [
        i64, f64, Quantity, &Quantity, Value, &Value, Complex64, &Complex64,
        Symbol, &Symbol, Constant, &Constant, Expr, &Expr
    ],
    exclude_specific = [],
    into = Pattern,
    out = Pattern,
    add = {
        // assert_eq!(
        //     lhs.shape(),
        //     rhs.shape(),
        //     "Tried to add two expressions of different shapes: {lhs}, {rhs}"
        // );

        Pattern::Node(Node::Branch(Branch::Add([Box::new(lhs), Box::new(rhs)])))
    },
    mul = {
        // assert!(
        //     lhs.shape().cols == lhs.shape().rows
        //         || (lhs.shape() == rhs.shape() && lhs.shape().is_vec()),
        //     "Matrix multiplication requires compatible shapes"
        // );

        Pattern::Node(Node::Branch(Branch::Mul([Box::new(lhs), Box::new(rhs)])))
    },
    div = { lhs * rhs.pow(-1) },
    sub = { lhs + (-rhs) },
    pow = {
        // assert!(
        //     lhs.shape().is_square_mat() || lhs.shape().is_scalar(),
        //     "Only square matrices can be raised to a power"
        // );

        // assert!(
        //     rhs.shape().is_square_mat() || rhs.shape().is_scalar(),
        //     "Only square matrices can be an exponent"
        // );

        // assert!(
        //     !(lhs.shape().is_square_mat() && rhs.shape().is_square_mat()),
        //     "Cannot raise a matrix to the power of another matrix yet"
        // );

        Pattern::Node(Node::Branch(Branch::Pow {
            base: Box::new(lhs),
            exp: Box::new(rhs),
        }))
    },
);

impl Neg for Pattern {
    type Output = Pattern;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl Neg for &Pattern {
    type Output = Pattern;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl Neg for Wildcard {
    type Output = Pattern;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl Neg for &Wildcard {
    type Output = Pattern;

    fn neg(self) -> Self::Output {
        -1 * self
    }
}

impl Into<usize> for Wildcard {
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for Wildcard {
    fn from(v: usize) -> Wildcard {
        Wildcard(v as u8)
    }
}

impl Into<usize> for RegisterId {
    fn into(self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for RegisterId {
    fn from(v: usize) -> RegisterId {
        RegisterId(v as u8)
    }
}

impl From<Expr<!>> for Pattern {
    fn from(value: Expr<!>) -> Self {
        const _: () =
            assert!(size_of::<Operand<!>>() == size_of::<Operand<Wildcard>>());

        // SAFETY: ! never happens, so its safe to just "discard" it,
        // and we know Operand is the same size with or without it
        unsafe { mem::transmute(value) }
    }
}

macro_rules! impl_unary_fn {
    ($fn:ident, $variant:ident, $name:literal) => {
        pub fn $fn(x: impl Into<Pattern>) -> Pattern {
            let expr = x.into();

            Pattern::Node(Node::Branch(Branch::$variant(Box::new(expr))).into())
        }
    };

    ($fn:ident, $variant:ident, $name:literal, square) => {
        pub fn $fn(x: impl Into<Pattern>) -> Pattern {
            let expr = x.into();
            // let shape = expr.shape();

            // assert!(
            //     shape.is_square_mat() || shape.is_scalar(),
            //     "Matrix-valued {} is only defined for square matrices",
            //     $name
            // );

            Pattern::Node(Node::Branch(Branch::$variant(Box::new(expr))).into())
        }
    };
}

impl_unary_fn!(sin, Sin, "sine", square);
impl_unary_fn!(cos, Cos, "cosine", square);
impl_unary_fn!(tan, Tan, "tangent", square);

impl_unary_fn!(asin, Asin, "inverse sine", square);
impl_unary_fn!(acos, Acos, "inverse cosine", square);
impl_unary_fn!(atan, Atan, "inverse tangent", square);

impl_unary_fn!(sinh, Sinh, "hyperbolic sine", square);
impl_unary_fn!(cosh, Cosh, "hyperbolic cosine", square);
impl_unary_fn!(tanh, Tanh, "hyperbolic tangent", square);

impl_unary_fn!(asinh, Asinh, "inverse hyperbolic sine", square);
impl_unary_fn!(acosh, Acosh, "inverse hyperbolic cosine", square);
impl_unary_fn!(atanh, Atanh, "inverse hyperbolic tangent", square);

impl_unary_fn!(real, Real, "real component of z");
impl_unary_fn!(imag, Imag, "imaginary component of z");
impl_unary_fn!(
    sign,
    Sign,
    "sign(z) = sign(real(z)) + i * sign(imag(z)), for a complex number z"
);

impl_unary_fn!(
    norm,
    Norm,
    "Norm of the given number, or Frobenius norm for matrices"
);

pub fn atan2(x: impl Into<Pattern>, y: impl Into<Pattern>) -> Pattern {
    let x = Box::new(x.into());
    let y = Box::new(y.into());

    // assert!(
    //     a.shape().is_scalar() && b.shape().is_scalar(),
    //     "atan2 is only defined for scalars"
    // );

    Pattern::Node(Node::Branch(Branch::Atan2 { x, y }).into())
}

pub fn log(base: impl Into<Pattern>, x: impl Into<Pattern>) -> Pattern {
    let base = Box::new(base.into());
    let x = Box::new(x.into());

    // assert!(
    //     base.shape().is_scalar(),
    //     "Logarithm is only defined for scalar bases"
    // );

    // assert!(
    //     x.shape().is_square_mat() || x.shape().is_scalar(),
    //     "Matrix-valued logarithm is only defined for square matrices"
    // );

    Pattern::Node(Node::Branch(Branch::Log { base, arg: x }).into())
}

pub fn ln(x: impl Into<Pattern>) -> Pattern {
    log(e, x)
}

pub fn exp(x: impl Into<Pattern>) -> Pattern {
    e.pow(x.into())
}

pub fn sqrt(x: impl Into<Pattern>) -> Pattern {
    x.into().pow(1 / 2)
}

pub fn cbrt(x: impl Into<Pattern>) -> Pattern {
    x.into().pow(1 / 3)
}

pub fn qtrt(x: impl Into<Pattern>) -> Pattern {
    x.into().pow(1 / 4)
}
