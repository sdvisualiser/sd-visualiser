use std::collections::{BTreeMap, BTreeSet};

use itertools::Itertools;
use num::rational::Ratio;
use sd_hyper::{
    concat_iter::concat_iter,
    graph::{GraphNode, HyperGraphError, NodeIndex, Port, PortIndex},
};
use thiserror::Error;
use tracing::{debug, debug_span};

use crate::graph::{HyperGraph, Op};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct WiredSlice {
    pub ops: Vec<(MonoidalWiredOp, Vec<NodeIndex>)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Slice {
    pub ops: Vec<(MonoidalOp, Vec<NodeIndex>)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Wiring {
    forward: Vec<BTreeSet<usize>>,
    backward: Vec<usize>,
}

impl Wiring {
    pub fn new(inputs: usize) -> Self {
        Wiring {
            forward: vec![BTreeSet::new(); inputs],
            backward: vec![],
        }
    }

    pub fn add_wire(&mut self, input: usize) {
        self.forward[input].insert(self.backward.len());
        self.backward.push(input);
    }

    pub fn to_slices(&self, prefix: &[NodeIndex]) -> Vec<Slice> {
        let mut slices = Slice::permutation_to_swaps(self.backward.clone(), prefix);
        let mut copy_slice = Vec::new();
        let mut is_empty = true;
        for x in &self.forward {
            let copies = x.len();
            if copies != 1 {
                is_empty = false;
            }
            copy_slice.push((MonoidalOp::Copy { copies }, prefix.to_vec()))
        }
        if !is_empty {
            slices.push(Slice { ops: copy_slice });
        }
        slices.reverse();
        slices
    }
}

impl WiredSlice {
    pub fn number_of_inputs(&self) -> usize {
        self.ops.iter().map(|(op, _)| op.number_of_inputs()).sum()
    }

    pub fn number_of_outputs(&self) -> usize {
        self.ops.iter().map(|(op, _)| op.number_of_outputs()).sum()
    }
}

impl Slice {
    pub fn number_of_inputs(&self) -> usize {
        self.ops.iter().map(|(op, _)| op.number_of_inputs()).sum()
    }

    pub fn number_of_outputs(&self) -> usize {
        self.ops.iter().map(|(op, _)| op.number_of_outputs()).sum()
    }

    pub fn permutation_to_swaps(mut permutation: Vec<usize>, prefix: &[NodeIndex]) -> Vec<Self> {
        let mut slices = Vec::new();

        let mut finished = false;

        while !finished {
            let mut slice_ops = Vec::new();
            finished = true; // We set finished back to false if we make a swap
            let mut i = 0; // Iterate through windows
            while i + 1 < permutation.len() {
                if permutation[i] <= permutation[i + 1] {
                    i += 1;
                    slice_ops.push((ID, prefix.to_vec()));
                } else {
                    finished = false;
                    slice_ops.push((MonoidalOp::Swap, prefix.to_vec()));
                    permutation.swap(i, i + 1);
                    i += 2;
                }
            }
            if i + 1 == permutation.len() {
                slice_ops.push((ID, prefix.to_vec()));
            }
            if !finished {
                // Slice is non trivial
                slices.push(Slice { ops: slice_ops });
            }
        }

        slices
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct MonoidalWiredGraph {
    pub inputs: usize,
    pub slices: Vec<WiredSlice>,
    pub wirings: Vec<Wiring>,
}

impl Default for MonoidalWiredGraph {
    fn default() -> Self {
        MonoidalWiredGraph {
            inputs: 0,
            slices: vec![],
            wirings: vec![Wiring::new(0)],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum MonoidalWiredOp {
    Id {
        port: Port,
    },
    Operation {
        inputs: Vec<Port>,
        op_name: Op,
    },
    Thunk {
        inputs: Vec<Port>,
        args: usize,
        body: MonoidalWiredGraph,
    },
}

impl MonoidalWiredOp {
    /// Returns number of inputs of an operation
    pub fn number_of_inputs(&self) -> usize {
        self.input_ports().len()
    }

    /// Returns number of outputs of an operation
    pub fn number_of_outputs(&self) -> usize {
        1
    }

    pub fn input_ports(&self) -> Vec<Port> {
        match self {
            MonoidalWiredOp::Id { port } => vec![*port],
            MonoidalWiredOp::Operation { inputs, .. } => inputs.clone(),
            MonoidalWiredOp::Thunk { inputs, .. } => inputs.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Default, Hash)]
pub struct MonoidalGraph {
    pub inputs: usize,
    pub slices: Vec<Slice>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum MonoidalOp {
    Copy {
        copies: usize,
    },
    Operation {
        inputs: usize,
        op_name: Op,
    },
    Thunk {
        args: usize,
        body: MonoidalGraph,
        expanded: bool,
    },
    Swap,
}

impl MonoidalOp {
    /// Returns number of cinputs of an operation
    pub fn number_of_inputs(&self) -> usize {
        match self {
            Self::Copy { .. } => 1,
            Self::Operation { inputs, .. } => *inputs,
            Self::Thunk { args, body, .. } => body.inputs - args,
            Self::Swap => 2,
        }
    }

    /// Returns number of outputs of an operation
    pub fn number_of_outputs(&self) -> usize {
        match self {
            Self::Copy { copies } => *copies,
            Self::Operation { .. } => 1,
            Self::Thunk { .. } => 1,
            Self::Swap => 2,
        }
    }
}

pub const ID: MonoidalOp = MonoidalOp::Copy { copies: 1 };

pub const DELETE: MonoidalOp = MonoidalOp::Copy { copies: 0 };

#[derive(Debug, Error, Clone)]
pub enum FromHyperError {
    #[error("Hypergraph contains no nodes")]
    EmptyGraph,

    #[error("Hypergraph error")]
    HyperGraphError(#[from] HyperGraphError),
}

// This can be made a lot nicer
impl MonoidalWiredGraph {
    pub fn from_hypergraph(
        graph: &HyperGraph,
        prefix: &[NodeIndex],
    ) -> Result<Self, FromHyperError> {
        // List of open ports we have left to process

        debug_span!("From hypergraph");
        debug!("To Process: {:?}", graph);

        // Separate the nodes into input nodes, output nodes, and other nodes by rank
        let (ranks, input_wires, output_wires) = {
            let (inputs, mut r, outputs) = graph.ranks_from_end();

            debug!("Inputs: {:?}", inputs);
            debug!("Ranks: {:?}", r);
            debug!("Outputs: {:?}", outputs);

            let input_wires = inputs
                .iter()
                .map(|x| graph.number_of_outputs(*x).unwrap())
                .sum();

            let output_wires = outputs
                .iter()
                .map(|x| graph.get_inputs(*x).unwrap().collect_vec())
                .concat();

            r.push(inputs);

            (r, input_wires, output_wires)
        };

        debug!("Input wires: {:?}", input_wires);
        debug!("Output wires: {:?}", output_wires);

        let mut open_wires: Vec<Port> = output_wires;

        let mut slices: Vec<WiredSlice> = Vec::new();
        let mut wirings: Vec<Wiring> = Vec::new();

        struct OpData {
            op: Option<MonoidalWiredOp>,
            outputs: usize,
            addr: Vec<NodeIndex>,
            node: NodeIndex,
            weight: Ratio<usize>,
        }
        for r in ranks {
            let mut ops: Vec<OpData> = Vec::new();

            for (i, port @ Port { node, index: _ }) in open_wires.iter().copied().enumerate() {
                if !r.contains(&node) {
                    ops.push(OpData {
                        op: Some(MonoidalWiredOp::Id { port }),
                        outputs: 1,
                        addr: prefix.to_vec(),
                        node,
                        weight: i.into(),
                    });
                }
            }

            for node in r.iter() {
                let node = *node;
                let addr = {
                    let mut temp = prefix.to_vec();
                    temp.push(node);
                    temp
                };
                let (sum, count) = open_wires
                    .iter()
                    .enumerate()
                    .filter_map(
                        |(i, Port { node: n, index: _ })| {
                            if &node == n {
                                Some((i, 1))
                            } else {
                                None
                            }
                        },
                    )
                    .fold((0, 0), |(x, y), (a, b)| (x + a, y + b));
                let weight = if count == 0 {
                    usize::MAX.into()
                } else {
                    Ratio::new_raw(sum, count)
                };
                let outputs = graph.number_of_outputs(node)?;
                let op = match graph.get(node)? {
                    GraphNode::Weight(op) => Some(MonoidalWiredOp::Operation {
                        inputs: graph.get_inputs(node)?.collect(),
                        op_name: *op,
                    }),
                    GraphNode::Input => None,
                    GraphNode::Output => None,
                    GraphNode::Thunk { args, body } => Some(MonoidalWiredOp::Thunk {
                        inputs: graph.get_inputs(node)?.collect(),
                        args: *args,
                        body: MonoidalWiredGraph::from_hypergraph(body, &addr)?,
                    }),
                };
                ops.push(OpData {
                    op,
                    outputs,
                    addr,
                    node,
                    weight,
                })
            }

            let number_of_out_ports = ops.iter().map(|data| data.outputs).sum();

            ops.sort_by_key(|data| data.weight);

            let out_nodes: BTreeMap<Port, usize> = concat_iter(ops.iter().map(|data| {
                (0..data.outputs).map(|index| Port {
                    node: data.node,
                    index: PortIndex(index),
                })
            }))
            .enumerate()
            .map(|(x, y)| (y, x))
            .collect();

            let mut wiring = Wiring::new(number_of_out_ports);

            for p in open_wires {
                wiring.add_wire(*out_nodes.get(&p).ok_or(HyperGraphError::UnknownPort(p))?);
            }

            open_wires = ops
                .iter()
                .map(|data| {
                    data.op
                        .as_ref()
                        .map(|x| x.input_ports())
                        .unwrap_or_default()
                })
                .concat();

            if let Some(ops) = ops
                .into_iter()
                .map(|data| Some((data.op?, data.addr)))
                .collect::<Option<Vec<_>>>()
            {
                slices.push(WiredSlice { ops });
            }

            wirings.push(wiring);
        }

        slices.reverse();
        wirings.reverse();

        Ok(MonoidalWiredGraph {
            inputs: input_wires,
            slices,
            wirings,
        })
    }
}

impl MonoidalWiredOp {
    pub fn to_monoidal_op(&self, node: &[NodeIndex]) -> Result<MonoidalOp, FromHyperError> {
        match self {
            MonoidalWiredOp::Id { .. } => Ok(ID),
            MonoidalWiredOp::Operation { inputs, op_name } => Ok(MonoidalOp::Operation {
                inputs: inputs.len(),
                op_name: *op_name,
            }),
            MonoidalWiredOp::Thunk { args, body, .. } => Ok(MonoidalOp::Thunk {
                args: *args,
                body: body.to_graph(node)?,
                expanded: true,
            }),
        }
    }
}

impl WiredSlice {
    pub fn to_slice(&self) -> Result<Slice, FromHyperError> {
        Ok(Slice {
            ops: self
                .ops
                .iter()
                .map(|(x, node)| Ok((x.to_monoidal_op(node)?, node.clone())))
                .collect::<Result<Vec<_>, FromHyperError>>()?,
        })
    }
}

impl MonoidalWiredGraph {
    pub fn to_graph(&self, prefix: &[NodeIndex]) -> Result<MonoidalGraph, FromHyperError> {
        let wiring_slices = self.wirings.iter().map(|w| w.to_slices(prefix));
        let slices: Vec<Slice> = wiring_slices
            .into_iter()
            .interleave(
                self.slices
                    .iter()
                    .map(|x| Ok(vec![x.to_slice()?]))
                    .collect::<Result<Vec<_>, FromHyperError>>()?,
            )
            .concat();
        Ok(MonoidalGraph {
            inputs: self.inputs,
            slices,
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(vec![0,1], vec![])]
    #[case(vec![1,0], vec![Slice { ops: vec![(MonoidalOp::Swap, vec![])]}])]
    #[case(vec![1,2,0], vec![Slice { ops: vec![(ID, vec![]),(MonoidalOp::Swap, vec![])]}, Slice { ops: vec![(MonoidalOp::Swap, vec![]), (ID, vec![])]}])]
    fn test_permutation(#[case] permutation: Vec<usize>, #[case] result: Vec<Slice>) -> Result<()> {
        assert_eq!(Slice::permutation_to_swaps(permutation, &vec![]), result);
        Ok(())
    }
}
