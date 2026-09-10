/* --------------------------------- TRAITS --------------------------------- */

use crate::{
    core::value::Value,
    model::{Variable, eq::Equation},
};

pub trait Solver {
    fn solve(eqs: Vec<Equation>) -> Vec<(Variable, Value)>;
}

/* --------------------------------- STRUCTS -------------------------------- */

// pub struct IpoptProblem {}
// pub struct IpoptSolver(ipopt::Ipopt<IpoptProblem>);
