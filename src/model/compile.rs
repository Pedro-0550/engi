use super::*;

#[derive(Debug, Clone)]
pub struct CompiledSystem {
    pub blocks: Vec<Vec<Expr>>,
}

impl System {
    pub fn compile(self) -> CompiledSystem {
        let mut equations = Vec::new();

        let connections = self.adjacency.borrow();

        /* -------------------------------------------------------------------------- */

        let mut visited = HashSet::new();

        for start_id in connections.keys() {
            if !visited.insert(start_id.clone()) {
                continue;
            }

            let mut component = vec![start_id.clone()];
            let mut stack = vec![start_id.clone()];

            while let Some(curr) = stack.pop() {
                if let Some(adjacent) = connections.get(&curr) {
                    for neighbor in adjacent {
                        if visited.insert(neighbor.clone()) {
                            component.push(neighbor.clone());
                            stack.push(neighbor.clone());
                        }
                    }
                }
            }

            let first_id = &component[0];
            let first_interface =
                &self.model(&first_id.path).interfaces[first_id.idx];

            for (i, first_conn) in first_interface.connectors.iter().enumerate()
            {
                match first_conn.condition {
                    Condition::Equal => {
                        for other_id in &component[1..] {
                            let other_interface = &self
                                .model(&other_id.path)
                                .interfaces[other_id.idx];
                            let other_conn = &other_interface.connectors[i];

                            // Associations are propagated on equality
                            let first_conn_id = ConnectorId {
                                interface: first_id.clone(),
                                idx: i,
                            };
                            let other_conn_id = ConnectorId {
                                interface: other_id.clone(),
                                idx: i,
                            };

                            let mut assoc = self.conn_association.borrow_mut();
                            let first_assoc = assoc.get(&first_conn_id);
                            let other_assoc = assoc.get(&other_conn_id);

                            match (first_assoc, other_assoc) {
                                (None, Some(other)) => {
                                    let other = other.clone();
                                    assoc.insert(first_conn_id, other);
                                }
                                (Some(first), None) => {
                                    let first = first.clone();
                                    assoc.insert(other_conn_id, first);
                                }
                                (Some(first), Some(other)) => {
                                    assert_eq!(*first, *other, "
                                        Associations on connectors with equality constraints propagate,
                                        and as such a chain of equality-connected cannot have different associations");
                                }
                                _ => (),
                            };

                            equations.push(relation! { first_conn.variable = other_conn.variable });
                        }
                    }
                    Condition::Conserved => {
                        let mut terms = component.iter().map(|id| {
                            let interface =
                                &self.model(&id.path).interfaces[id.idx];
                            interface.connectors[i].variable
                        });

                        let mut term_sum = Expr::new();
                        let init = term_sum.push(terms.next().unwrap().into());
                        let root_term = terms.fold(init, |a, v| {
                            let b = term_sum.push(v.into());
                            term_sum.edit().add(a, b)
                        });
                        term_sum.set_root(root_term);

                        equations.push(relation!(term_sum = 0));
                    }
                }
            }
        }

        fn collect_equations(
            model: &mut AssembledModel,
            equations: &mut Vec<Equation>,
        ) {
            equations.append(&mut model.equations);

            for submodel in &mut model.submodels {
                collect_equations(submodel, equations);
            }
        }

        for model in &mut *self.models.borrow_mut() {
            collect_equations(model, &mut equations);
        }

        let var_assoc = self.var_association.borrow().clone();
        let conn_assoc = self.conn_association.borrow().clone();

        let bindings = var_assoc
            .into_iter()
            .filter_map(|(var_id, assoc)| match assoc {
                Associated::Binding(expr) => Some((
                    self.model(&var_id.path).variables[var_id.idx].symbol(),
                    expr.simplified(),
                )),
                _ => None,
            })
            .chain(conn_assoc.into_iter().filter_map(|(conn_id, assoc)| {
                match assoc {
                    Associated::Binding(expr) => Some((
                        self.model(&conn_id.interface.path).interfaces
                            [conn_id.interface.idx]
                            .connectors[conn_id.idx]
                            .variable()
                            .symbol(),
                        expr.simplified(),
                    )),
                    _ => None,
                }
            }))
            .collect();

        id!(ResidualId);

        #[derive(Copy, Hash, Eq, PartialEq, Clone, PartialOrd, Ord)]
        enum Node {
            Residual(ResidualId),
            Variable(Symbol),
        }

        let mut residuals: Indexed<Vec<_>, ResidualId> = equations
            .par_iter()
            .filter_map(|eq| {
                let mut resid = eq.residual();

                loop {
                    let Some(step) = resid.substituted(&bindings) else {
                        break;
                    };
                    resid = step;
                }

                resid = resid.simplified();

                if try { resid.as_single()?.as_leaf()?.as_quantity()? }
                    .is_some_and(|x| *x.value() == 0)
                {
                    None
                } else {
                    Some(resid)
                }
            })
            .collect::<Vec<_>>()
            .into();

        let mut incidence = UnGraphMap::<Node, ()>::new();

        for (id, resid) in residuals.iter() {
            let r = incidence.add_node(Node::Residual(id));

            for symb in resid.symbols() {
                let v = incidence.add_node(Node::Variable(symb));
                incidence.add_edge(r, v, ());
            }
        }

        let matching = maximum_matching(&incidence);

        // if matching. {
        //     panic!(
        //         "Not every equation could be assigned to a variable: {} equations, {} matched
        //         Unmatched equations: {:#?}
        //         This indicates the system is overdetermined.
        //         You need to unbind a variable so the system has a new degree of freedom.",
        //         incidence.left_count(),
        //         matching.size(),
        //         matching.unmatched_left(incidence.left_nodes()).map(|x| x.to_string()).collect::<Vec<_>>().join(", ")
        //     );
        // }

        // if matching.size() != incidence.right_count() {
        //     panic!(
        //         "Not every variable could be assigned to an equation: {} variables, {} matched
        //         Unmatched variables: {:#?}
        //         This indicates the system is underdetermined.
        //         You should verify if you entered all the equations, or bind a constant value to another variable.",
        //         incidence.right_count(),
        //         matching.size(),
        //         matching.unmatched_right(incidence.right_nodes()).map(|x| x.symbol().to_string()).collect::<Vec<_>>().join(", ")
        //     );
        // }

        if !matching.is_perfect() {
            panic!()
        }

        let mut dependency_graph = DiGraphMap::<ResidualId, ()>::new();

        for (a, b) in matching.edges() {
            let (sym, resid) = match (a, b) {
                (Node::Residual(r), Node::Variable(s)) => (s, r),
                (Node::Variable(s), Node::Residual(r)) => (s, r),
                _ => unreachable!(),
            };

            dependency_graph.add_node(resid);

            for neighbor in incidence.neighbors(Node::Variable(sym)) {
                let Node::Residual(neighbor) = neighbor else {
                    unreachable!();
                };

                if neighbor == resid {
                    continue;
                }
                dependency_graph.add_edge(resid, neighbor, ());
            }
        }

        CompiledSystem {
            blocks: tarjan_scc(&dependency_graph)
                .iter()
                .map(|block| {
                    block
                        .iter()
                        .map(|id| mem::take(&mut residuals[*id]))
                        .collect_vec()
                })
                .collect_vec(),
        }
    }
}
