# Engi

**Rust Modeling Framework**

This crate provides a mathematical modeling framework tailored for engineering, with an easy-to-use, composable, and fast user API.

Disclaimer: This crate is very WIP and unstable as of now. I would still encourage you to give it a try, and open issues if you have any suggestions or find any bugs. I will release a version on crates.io as soon as reasonable.

If you already know the basics of modeling, head to [examples](README.md#example)

In essence this allows you to define systems as the composition of individual models. Think Modelica or ModelingFramework.jl.

A model is just an individual component like `WaterTower`, which holds its own variables: `water_level` and `water_volume`; connectors: `inlet` and `outlet`; and relationships between them such as `water_level = water_volume / 8m²` and `water_level > 0`. This is a simple example but you can simulate even really complex circuits. If you think SPICE is cumbersome and slow to iterate then this is a good alternative.

In such a system, there are no inputs or outputs. Instead you define the relevant variables, bind the known values, and solve for everything else. You could solve a BJT circuit for its saturation current for example.

Expressions are JIT compiled using Cranelift for maximum speed when evaluating. Complex numbers are supported even using solvers that don't normally support them, by splitting expressions into real and imaginary parts analytically.

Relationships between models are created using interfaces. An interface is a list of connections which encapsulates the transfer of some quantity between models. An example interface is an electrical pin, containing a voltage connector and a current connector. Connectors can have equality conditions or conservation conditions. In equality, every related connector has the same value, and in conservation the sum of every related connector must be zero (think KCL).

# Features

Implemented:

- Units & dimensional analysis
- First-class complex numbers & matrices
- NLopt solver backend
- Steady-state modeling
- Simple expression syntax
- Boilerplate macros
- JIT expression compiler
- Non-trivial expression simplification
- Connectors (both conserved and equality) and interfaces

Planned:

- Constraints: Restrict a solution to one that satisfies a set of arbitrary contraints, beyond simple bounds
- Dynamic system modeling: Allow modeling dynamic systems using differential equations, this is probably the biggest missing feature
- Solving for a set: Allow solving for a set of possible values instead of a single one. Useful to characterize all operation regimes in a DC power converter, based on a set/interval of supported mains voltages for example.
- Solution reiteration: After solving for the first time, Allow the user to redo the solution with different parameters while keeping the previous values as guesses to speed up convergence.

# Example

```rust
#[derive(Interface)]
struct Pin {
    #[connect(unit = V, cond = Condition::Equal)]
    voltage: Connector,

    #[connect(unit = A, cond = Condition::Conserved)]
    current: Connector
}

#[derive(Model)]
struct Port {
    #[interface]
    pos: Pin,
    #[interface]
    neg: Pin,

    #[var(unit = V)]
    variable: Variable,

    #[var(unit = V)]
    current: Variable,
}

impl Relations for Port {
    fn equations(&self) -> Vec<Equation> {
        let Self { pos, neg, voltage, current } = self;
        relations! {
            voltage = pos.voltage - neg.voltage;
            current = pos.current;
            pos.current = neg.current;
        }
    }
}

#[derive(Model)]
struct Resistor {
    #[var(unit = Ω)]
    resistance: Variable,

    // submodels can be used to bundle common/required implementations
    #[model]
    port: Port
}

impl Relations for Resistor {
    fn equations(&self) -> Vec<Equation> {
        let Self { port, resistance } = self;
        // This macro dosent parse expr syntax, it only removes the boilerplate of constructing an equation using Equation::new
        relations! {
            // ohms law, V = I * R
            port.voltage = port.current * resistance;
        }
    }
}

#[derive(Model)]
struct Ground {
    #[interface]
    pin: Pin
}

impl Relations for Ground {
    fn equations(&self) -> Vec<Equation> {
        let Self { pin } = self;

        relations! {
            pin.voltage = 0;
        }
    }
}

fn main() {
   let system = System::new();
   let r = system.add(Resistor::new("r1"));
   let gnd = system.add(Ground::new("gnd"));
   let supply = system.add(Port::new("supply"));

   supply.voltage.bind(5 * V);
   supply.pos.connect(r.pos);
   [supply.neg, r.neg].connect(gnd.pin);

   let solution = system.solve();
   let r_sol = solution.get(r);

   println!("The current through the Resistor is {}", r_sol.port.current);
}

```
