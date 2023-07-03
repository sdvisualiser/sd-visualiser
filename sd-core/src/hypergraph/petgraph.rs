use std::collections::HashMap;

use petgraph::graph::NodeIndex;
#[cfg(test)]
use serde::Serialize;

use super::{Edge, Graph, Node};

pub type PetGraph<V, E> = petgraph::Graph<PetNode<V, E>, usize>;

#[cfg_attr(test, derive(Serialize))]
pub enum PetNode<V, E> {
    Edge(E),
    Operation(V),
    Thunk(PetGraph<V, E>),
}

pub fn to_pet<V, E>(hypergraph: &impl Graph<NodeWeight = V, EdgeWeight = E>) -> PetGraph<V, E>
where
    V: Clone,
    E: Clone,
{
    let mut graph = petgraph::Graph::new();

    // Maps hyperedges to petgraph nodes.
    let mut edge_map: HashMap<Edge<V, E>, NodeIndex> = HashMap::new();

    // Need to do this in case there are any inputs that are immediately discarded.
    for edge in hypergraph.graph_inputs() {
        edge_map.insert(
            edge.clone(),
            graph.add_node(PetNode::Edge(edge.weight().clone())),
        );
    }

    for node in hypergraph.nodes() {
        let pet_node = match &node {
            Node::Operation(op) => graph.add_node(PetNode::Operation(op.weight().clone())),
            Node::Thunk(thunk) => graph.add_node(PetNode::Thunk(to_pet(thunk))),
        };

        for (i, edge) in node.inputs().enumerate() {
            let pet_edge = edge_map
                .entry(edge)
                .or_insert_with_key(|edge| graph.add_node(PetNode::Edge(edge.weight().clone())));
            graph.add_edge(*pet_edge, pet_node, i);
        }
        for (i, edge) in node.outputs().enumerate() {
            let pet_edge = edge_map
                .entry(edge)
                .or_insert_with_key(|edge| graph.add_node(PetNode::Edge(edge.weight().clone())));
            graph.add_edge(pet_node, *pet_edge, i);
        }
    }

    graph
}