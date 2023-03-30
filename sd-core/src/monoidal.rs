use std::rc::Rc;

use crate::language::grammar::Variable;

#[derive(Clone, Debug)]
pub struct Slice {
    pub ops: Vec<MonoidalOp>,
}

#[derive(Clone, Debug)]
pub struct MonoidalGraph {
    pub inputs: Vec<Variable>,
    pub slices: Vec<Slice>,
}

#[derive(Clone, Debug)]
pub enum MonoidalOp {
    Id {
        name: Rc<Variable>,
    },
    Copy {
        input: Rc<Variable>,
        copies: usize,
    },
    Delete,
    // Tuple {
    //     inputs: usize,
    //     name: Rc<Variable>,
    // },
    // Untuple {
    //     outputs: Vec<Rc<Variable>>,
    // },
    Operation {
        inputs: usize,
        op_name: Rc<Variable>,
        name: Rc<Variable>,
    },
    Thunk {
        inputs: usize,
        args: Vec<Rc<Variable>>,
        body: MonoidalGraph,
        name: Rc<Variable>,
    },
    Swap {
        first: Rc<Variable>,
        second: Rc<Variable>,
    },
}

impl MonoidalOp {
    pub fn number_of_inputs(&self) -> usize {
        match self {
            Self::Id { .. } => 1,
            Self::Copy { .. } => 1,
            Self::Delete => 1,
            Self::Operation { inputs, .. } => *inputs,
            Self::Thunk { inputs, .. } => *inputs,
            Self::Swap { .. } => 2,
        }
    }

    /// Returns number of outputs of an operation
    pub fn outputs(&self) -> Vec<Rc<Variable>> {
        match self {
            MonoidalOp::Id { name } => vec![name.clone()],
            MonoidalOp::Copy { input, copies } => {
                std::iter::repeat(input.clone()).take(*copies).collect()
            }
            MonoidalOp::Delete => vec![],
            // MonoidalOp::Tuple { name, .. } => vec![name.clone()],
            // MonoidalOp::Untuple { outputs } => outputs.iter().cloned().collect(),
            MonoidalOp::Operation { name, .. } => vec![name.clone()],
            MonoidalOp::Thunk { name, .. } => vec![name.clone()],
            MonoidalOp::Swap { first, second } => vec![second.clone(), first.clone()],
        }
    }
}

impl MonoidalGraph {}

// impl From<MonoidalGraph> for Expr {
//     fn from(term: MonoidalGraph) -> Self {

//     }
// }
