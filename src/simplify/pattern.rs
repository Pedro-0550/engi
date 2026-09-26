/* --------------------------------- STRUCTS -------------------------------- */

use std::collections::HashMap;

use crate::{
    expr::{Expr, NodeKind, domain::Domain, shape::Shape},
    simplify::{ClassId, EquivalencyClass, EquivalencyGraph, Substitution},
    symbol::constants::c,
    units::Quantity,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reg(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Wildcard(usize);

pub struct Program {
    code: Vec<Instruction>,
    allocations: HashMap<Wildcard, Reg>,
    register_count: usize,
}

pub struct Machine;

pub struct Condition {
    over: Box<[Wildcard]>,
    f: fn(ConditionContext<'_>) -> bool,
}

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
    Bind { kind: NodeKind, target: Reg, out: Reg },
    Compare { a: Reg, b: Reg },
    If { f: fn(ConditionContext<'_>) -> bool },
}

#[derive(PartialEq, Clone, Eq)]
pub enum Pattern {
    Wildcard(Wildcard),
    Node(NodeKind, Box<[Pattern]>),
}

impl<'a> ConditionContext<'a> {
    pub fn class(&self, wildcard: &Wildcard) -> Option<&'a EquivalencyClass> {
        let reg = self.allocations.get(wildcard)?;
        let class_id = self.registers[reg.0];

        Some(&self.graph.classes[class_id.0])
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
                        state.instructions.push(Instruction::If { f: cond.f });
                    }
                }
            }
            Pattern::Node(kind, children) => {
                let out = state.next;
                state.next.0 += children.len();

                state.instructions.push(Instruction::Bind {
                    kind: *kind,
                    target,
                    out,
                });

                for (i, child) in children.iter().enumerate() {
                    child.compile_inner(Reg(out.0 + i), conditions, state);
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
                Instruction::Bind { kind, target, out } => {
                    let target_id = registers[target.0];
                    let target_class = &graph.classes[graph.find(target_id).0];

                    for node_id in &target_class.nodes {
                        let node = &graph.nodes[node_id.0];

                        if node.kind == *kind {
                            let mut regs = registers.clone();

                            for (i, &child_class) in
                                node.children.iter().enumerate()
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
