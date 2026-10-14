# Engi
**Rust Modeling Framework**

This crate provides a mathematical modeling framework tailored for engineering, with an easy-to-use, composite, and fast user API.

Disclaimer: This crate is very WIP and unstable as of now. I would still encourage you to give it a try, and open issues if you have any suggestions or find any bugs. I will release a version on crates.io as soon as reasonable.

If you already know the basics of modeling, head to [examples](README.md#example)

In essence this allows you to define systems as the composition of individual models. Think Modelica or ModelingFramework.jl. 

A model is just an individual component like `WaterTower`, which holds its own variables: `water_level` and `water_volume`; connectors: `inlet` and `outlet`; and relationships between them such as `water_level = water_volume / 8m²` and `water_level > 0`. This is a simple example but you can simulate even really complex circuits. If you think SPICE is cumbersome and slow to iterate then this is a good alternative.

In such a system, there are no inputs or outputs. Instead you define the relevant variables, bind the known values, and solve for everything else. You could solve a BJT circuit for its saturation current for example. 

Expressions are JIT compiled using Cranelift for maximum speed when evaluating. Complex numbers are supported even using solvers that don't normally support them, by splitting expressions into real and imaginary parts analytically.

# Features
Implemented:
- Units & dimensional analysis
- First-class complex numbers & matrices
- NLopt solver backend
- Steady-state modeling
- Simple expression syntax
- Boilerplate macros
- JIT expression compiler
  
Planned:
- Constraints: Restrict a solution to one that satisfies a set of arbitrary contraints, beyond simple bounds
- Dynamic system modeling: Allow modeling dynamic systems using differential equations, this is probably the biggest missing feature
- Solving for a set: Allow solving for a set of possible values instead of a single one. Useful to characterize all operation regimes in a DC power converter, based on a set/interval of supported mains voltages for example.

# Example
TODO
