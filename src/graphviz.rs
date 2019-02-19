// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::fndata::FnData;

use rustc_graphviz as graphviz;
use rustc_graphviz::{Labeller, GraphWalk, Style};

use std::iter::FromIterator;

use rls_data::Id;


// Graphviz interaction.
//
// We use NodeIds to identify nodes in the graph to Graphviz. We label them by
// looking up the name for the id in self.functions. Edges are the union of
// static and dynamic calls. We don't label edges, but potential calls due to
// dynamic dispatch get dotted edges.
//
// Invariants: all edges must be beween nodes which are in self.functions.
//             post_process must have been called (i.e., no decls left in the graph)

// Whether a call certainly happens (e.g., static dispatch) or only might happen
// (e.g., all possible receiving methods of dynamic dispatch).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CallKind {
    Definite,
    Potential,
}

// An edge in the callgraph, only used with graphviz.
pub type Edge = (Id, Id, CallKind);

// Issues ids, labels, and styles for graphviz.
impl<'a> Labeller<'a> for FnData {
    type Node = Id;
    type Edge = Edge;

    fn graph_id(&'a self) -> graphviz::Id<'a> {
        graphviz::Id::new(format!("Callgraph_for_{}", self.crate_name)).unwrap()
    }

    fn node_id(&'a self, n: &Id) -> graphviz::Id<'a> {
        graphviz::Id::new(format!("n_{}_{}", n.krate, n.index)).unwrap()
    }

    fn node_label(&'a self, n: &Id) -> graphviz::LabelText<'a> {
        // To find the label, we just lookup the function name.
        graphviz::LabelText::label(&*self.functions[n])
    }

    fn edge_style(&'a self, e: &Edge) -> Style {
        match e.2 {
            CallKind::Definite => Style::None,
            CallKind::Potential => Style::Dotted,
        }
    }
}

// Drives the graphviz visualisation.
impl<'a> GraphWalk<'a> for FnData {
    type Node = Id;
    type Edge = Edge;

    fn nodes(&'a self) -> graphviz::Nodes<'a, Id> {
        graphviz::Nodes::from_iter(self.functions.keys().cloned())
    }

    fn edges(&'a self) -> graphviz::Edges<'a, Edge> {
        let static_iter = self.static_calls.iter().map(|&(ref f, ref t)| (f.clone(),
                                                                          t.clone(),
                                                                          CallKind::Definite));
        let dyn_iter = self.dynamic_calls.iter().map(|&(ref f, ref t)| (f.clone(),
                                                                        t.clone(),
                                                                        CallKind::Potential));
        graphviz::Edges::from_iter(static_iter.chain(dyn_iter))
    }

    fn source(&'a self, &(from, _, _): &Edge) -> Id {
        from
    }

    fn target(&'a self, &(_, to, _): &Edge) -> Id {
        to
    }
}

