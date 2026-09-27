/* --------------------------------- STRUCTS -------------------------------- */

use std::{collections::HashMap, ops::Neg, rc::Rc};

// use cranelift::codegen::ir::Constant;
use kinded::Kinded;
use num::{complex::Complex64, pow::Pow};

use crate::{
    core::{util::impl_op_permutations, value::Value},
    expr::{
        Expr,
        domain::Domain,
        shape::Shape,
        tree::{Branch, Node},
    },
    model::{Connector, ConnectorBuilder, Variable, VariableBuilder},
    simplify::{
        ClassId, EquivalencyClass, EquivalencyExpr, EquivalencyGraph,
        EquivalencyNode, EquivalencyNodeStructure, Leaf, Substitution,
    },
    symbol::{
        Symbol,
        constants::{Constant, e},
    },
    units::{Quantity, Unit},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Wildcard(pub(crate) usize);

pub struct Program {
    code: Vec<Instruction>,
    allocations: HashMap<Wildcard, Reg>,
    register_count: usize,
}

pub struct Machine;

#[derive(Clone)]
pub struct Condition {
    pub over: Box<[Wildcard]>,
    pub f: Rc<dyn Fn(ConditionContext<'_>) -> bool>,
}

#[derive(Clone)]
pub struct Rule {
    pub from: Pattern,
    pub to: Pattern,
    pub conds: Box<[Condition]>,
}

struct CompilerState {
    instructions: Vec<Instruction>,
    next: Reg,
    allocations: HashMap<Wildcard, Reg>,
}

pub struct ConditionContext<'a> {
    registers: &'a [ClassId],
    allocations: &'a HashMap<Wildcard, Reg>,
    graph: &'a EquivalencyGraph,
}

pub enum Instruction {
    Bind { structure: EquivalencyNodeStructure, target: Reg, out: Reg },
    Compare { a: Reg, b: Reg },
    If { f: Rc<dyn Fn(ConditionContext<'_>) -> bool> },
}

#[derive(PartialEq, Clone, Eq)]
pub enum Pattern {
    Wildcard(Wildcard),
    Node(Node<Box<Pattern>>),
}

impl<'a> ConditionContext<'a> {
    pub fn class(&self, wildcard: Wildcard) -> Option<&'a EquivalencyClass> {
        let reg = self.allocations.get(&wildcard)?;
        let class_id = self.registers[reg.0];

        Some(&self.graph.classes[class_id.0])
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
            next: Reg(1),
            allocations: HashMap::new(),
        };

        Self::compile_inner(
            self,
            Reg(0),
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
        target: Reg,
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
                    let mut children = node.children();
                    state.next.0 += children.by_ref().count();

                    state.instructions.push(Instruction::Bind {
                        structure: EquivalencyNodeStructure::Branch(
                            branch.kind(),
                        ),
                        target,
                        out,
                    });

                    for (i, child) in children.enumerate() {
                        child.compile_inner(Reg(out.0 + i), conditions, state);
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
        let initial_regs = vec![root; program.register_count];
        let mut stack = vec![(0, initial_regs)];

        while let Some((pc, registers)) = stack.pop() {
            if pc >= program.code.len() {
                out.push(Substitution {
                    bindings: program
                        .allocations
                        .iter()
                        .map(|(wildcard, reg)| (*wildcard, registers[reg.0]))
                        .collect(),
                });

                continue;
            }

            match &program.code[pc] {
                Instruction::Bind { structure, target, out } => {
                    let target_id = registers[target.0];
                    let target_class = &graph.classes[graph.find(target_id).0];

                    for node_id in &target_class.exprs {
                        let expr = &graph.exprs[node_id.0];

                        if expr.node.structure() == *structure {
                            let mut regs = registers.clone();

                            for (i, &child_class) in
                                expr.node.children().enumerate()
                            {
                                regs[out.0 + i] = child_class;
                            }

                            stack.push((pc + 1, regs));
                        }
                    }
                }
                Instruction::Compare { a, b } => {
                    let a = registers[a.0];
                    let b = registers[b.0];

                    if graph.find(a) == graph.find(b) {
                        stack.push((pc + 1, registers))
                    }
                }
                Instruction::If { f } => {
                    let ctx = ConditionContext {
                        allocations: &program.allocations,
                        graph,
                        registers: &registers,
                    };

                    if f(ctx) {
                        stack.push((pc + 1, registers))
                    }
                }
            }
        }
    }
}

impl<T> From<&T> for Pattern
where
    Pattern: From<T>,
{
    default fn from(value: &T) -> Self {
        value.clone().into()
    }
}

impl From<&Pattern> for Pattern {
    fn from(expr: &Pattern) -> Self {
        expr.clone()
    }
}

impl From<f64> for Pattern {
    fn from(v: f64) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Quantity(v.into())))
    }
}

impl From<i64> for Pattern {
    fn from(v: i64) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Quantity(v.into())))
    }
}

impl From<Value> for Pattern {
    fn from(v: Value) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Quantity(v * Unit::Unitless)))
    }
}

impl From<Complex64> for Pattern {
    fn from(v: Complex64) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Quantity(v * Unit::Unitless)))
    }
}

impl From<Wildcard> for Pattern {
    fn from(v: Wildcard) -> Self {
        Pattern::Wildcard(v)
    }
}

impl From<Symbol> for Pattern {
    fn from(v: Symbol) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Symbol(v)))
    }
}

impl From<Constant> for Pattern {
    fn from(v: Constant) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Constant(v)))
    }
}

impl From<Quantity> for Pattern {
    fn from(v: Quantity) -> Self {
        Pattern::Node(Node::Leaf(Leaf::Quantity(v)))
    }
}

impl_op_permutations!(
    types = [
        i64, f64, Complex64, &Complex64, Quantity, &Quantity, Value, &Value,
        Constant, &Constant, Symbol, &Symbol, Wildcard, &Wildcard, Pattern,
        &Pattern
    ],
    exclude_permutations = [
        i64, f64, Quantity, &Quantity, Value, &Value, Complex64, &Complex64,
        Symbol, &Symbol, Constant, &Constant
    ],
    exclude_specific = [],
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
    partial_eq = { lhs == rhs }
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

pub fn atan2(a: impl Into<Pattern>, b: impl Into<Pattern>) -> Pattern {
    let a = Box::new(a.into());
    let b = Box::new(b.into());

    // assert!(
    //     a.shape().is_scalar() && b.shape().is_scalar(),
    //     "atan2 is only defined for scalars"
    // );

    Pattern::Node(Node::Branch(Branch::Atan2 { a, b }).into())
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
