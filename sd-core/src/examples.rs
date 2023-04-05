use crate::{
    language::grammar::ActiveOp,
    monoidal::{MonoidalGraph, MonoidalOp, Slice, ID},
};

pub fn copy() -> MonoidalGraph {
    use MonoidalOp::*;

    MonoidalGraph {
        inputs: 2,
        slices: vec![
            Slice {
                ops: vec![(Copy { copies: 2 }, vec![]), ID],
            },
            Slice {
                ops: vec![(Copy { copies: 2 }, vec![]), ID, ID],
            },
        ],
    }
}

pub fn thunk() -> MonoidalGraph {
    use MonoidalOp::*;

    let plus = MonoidalGraph {
        inputs: 2,
        slices: vec![Slice {
            ops: vec![(
                Operation {
                    inputs: 2,
                    op_name: ActiveOp::Plus(()).into(),
                },
                vec![],
            )],
        }],
    };

    MonoidalGraph {
        inputs: 3,
        slices: vec![Slice {
            ops: vec![
                (
                    Thunk {
                        args: 1,
                        body: plus,
                    },
                    vec![],
                ),
                (
                    Operation {
                        inputs: 2,
                        op_name: ActiveOp::Plus(()).into(),
                    },
                    vec![],
                ),
            ],
        }],
    }
}
