/* --------------------------------- STRUCTS -------------------------------- */

use std::{collections::HashMap, mem, ops::Neg, rc::Rc};

use ahash::AHashMap;
use derive_more::Into;
// use cranelift::codegen::ir::Constant;
use kinded::Kinded;
use num::{complex::Complex64, pow::Pow};

use crate::{
    core::{
        index::{Indexed, Size},
        util::impl_op_permutations,
        value::Value,
    },
    expr::{
        Expr, Operand, OperandKind, Order,
        dag::{
            self, Dag, LeafId, LeafKind, MatrixId, OperationId, OperationKind,
        },
        domain::Domain,
        shape::Shape,
    },
    id,
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    simplify::{
        ClassId, EquivalencyClass, EquivalencyDag, EquivalencyGraph, Leaf,
        Substitution,
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

id!(Wildcard, u8);
id!(RegisterId);

pub struct Program {
    code: Vec<Instruction>,
    allocations: Allocations,
    register_count: Size,
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
    /// Bind tries to match the given structure against the nodes in the target register's pointed class.
    /// On the first structure match, it pushes this operand's children classes to the registers starting at out.
    /// Backtracks if there are no matches.
    Bind { structure: Structure, target: RegisterId, out: RegisterId },
    /// Compare checks if the classes in a and b are of equal parent, i.e. they are the same class.
    /// If not, backtracks
    Compare { a: RegisterId, b: RegisterId },
    /// If runs the given condition function, backtracks if it returns false.
    If { f: fn(ConditionContext<'_>) -> bool },
}

pub type Pattern = Expr<Wildcard>;

/// Encapusalates all information required to check if two operands can be bound:
/// * Leaves need to be equal, and since leaves are interned we can just compile tailored for each expr and compare IDs
/// * Matrices only need to be of the same shape
/// * Operations only need to be of the same kind
pub enum Structure {
    Leaf(LeafKind, LeafId),
    Matrix(Shape),
    Operation(OperationKind),
}

/* -------------------------------------------------------------------------- */

impl Structure {
    fn kind(&self) -> OperandKind {
        match self {
            Structure::Leaf(leaf_kind, _) => OperandKind::Leaf(*leaf_kind),
            Structure::Matrix(shape) => OperandKind::Matrix,
            Structure::Operation(operation_kind) => {
                OperandKind::Operation(*operation_kind)
            }
        }
    }
}

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
    pub(crate) fn compile(
        &self,
        conditions: &[Condition],
        tailored_to: &mut EquivalencyDag,
    ) -> Program {
        let mut state = CompilerState {
            instructions: Vec::new(),
            next: RegisterId(1),
            allocations: [None; WILDCARD_COUNT].into(),
        };

        self.compile_inner(
            self.root().expect("Cannot compile a rootless pattern.. what are you even trying to do"),
            RegisterId(0),
            &conditions,
            &mut state,
            tailored_to,
        );

        Program {
            code: state.instructions,
            allocations: state.allocations,
            register_count: state.next.0,
        }
    }

    fn compile_inner(
        &self,
        op: Operand<Wildcard>,
        target: RegisterId,
        conditions: &[Condition],
        state: &mut CompilerState,
        tailored_to: &mut EquivalencyDag,
    ) {
        match self.fetch(op) {
            Operand::Wildcard(w) => {
                if let Some(allocated) = state.allocations[w] {
                    state
                        .instructions
                        .push(Instruction::Compare { a: allocated, b: target });
                } else {
                    state.allocations[w] = Some(target);
                }
                for cond in conditions {
                    if cond.over.iter().all(|w| state.allocations[*w].is_some())
                    {
                        state.instructions.push(Instruction::If { f: cond.f });
                    }
                }
            }

            Operand::Leaf(l) => {
                let leaf_id = tailored_to.leaves.push(l.clone());
                state.instructions.push(Instruction::Bind {
                    structure: Structure::Leaf(l.kind(), leaf_id),
                    target,
                    out: state.next,
                });
            }

            Operand::Operation(o) => {
                let out = state.next;
                let len = o.len();
                state.next.0 += len as u16;

                state.instructions.push(Instruction::Bind {
                    structure: Structure::Operation(o.kind()),
                    target,
                    out,
                });

                for i in 0..len {
                    let child = o.operand(i).unwrap();
                    self.compile_inner(
                        *child,
                        RegisterId(out.0 + i as u16),
                        conditions,
                        state,
                        tailored_to,
                    );
                }
            }

            Operand::Matrix(m) => {
                let out = state.next;
                state.next.0 += m.elements().len() as u16;

                state.instructions.push(Instruction::Bind {
                    structure: Structure::Matrix(m.shape()),
                    target,
                    out,
                });

                for (i, child) in m.elements().iter().enumerate() {
                    self.compile_inner(
                        *child,
                        RegisterId(out.0 + i as u16),
                        conditions,
                        state,
                        tailored_to,
                    );
                }
            }
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
                    let operands = &target_class.operands_by_kind
                        [structure.kind().id() as usize];

                    for op in operands {
                        let matches = match (op, structure) {
                            (
                                Operand::Leaf(id_op),
                                Structure::Leaf(_, id_s),
                            ) => id_op == id_s,
                            (Operand::Matrix(id), Structure::Matrix(shape)) => {
                                graph.dag.matrices[*id].shape() == *shape
                            }
                            (
                                Operand::Operation(id),
                                Structure::Operation(kind),
                            ) => graph.dag.operations[*id].kind() == *kind,
                            _ => false,
                        };

                        if matches {
                            let mut i = 0;
                            match op {
                                Operand::Matrix(m) => {
                                    for el in graph.dag.matrices[*m].elements()
                                    {
                                        let out = RegisterId(bind_out.0 + i);
                                        registers[out] = *el;

                                        i += 1;
                                    }
                                }
                                Operand::Operation(op) => {
                                    graph.dag.operations[*op].for_each_operand(
                                        |child| {
                                            let out =
                                                RegisterId(bind_out.0 + i);
                                            registers[out] = *child;

                                            i += 1;
                                        },
                                    );
                                }
                                _ => (),
                            }

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
