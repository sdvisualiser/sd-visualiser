use std::sync::Arc;

use derivative::Derivative;
use itertools::Either;

use crate::{
    codeable::{Code, Codeable},
    common::Matchable,
    hypergraph::{
        generic::{Ctx, Edge, EdgeWeight, Node, Operation, OperationWeight, Thunk, ThunkWeight},
        subgraph::ExtensibleEdge,
        traits::{EdgeLike, Graph, NodeLike, WithWeight},
    },
    weak_map::WeakMap,
};

////////////////////////////////////////////////////////////////

#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Eq(bound = ""),
    PartialEq(bound = ""),
    Hash(bound = ""),
    Debug(bound = "")
)]
pub struct CollapseGraph<G: Graph> {
    graph: G,
    expanded: Arc<WeakMap<Thunk<G::Ctx>, bool>>,
}

impl<G: Graph> CollapseGraph<G> {
    pub fn new(graph: G, expanded: WeakMap<Thunk<G::Ctx>, bool>) -> Self {
        Self {
            graph,
            expanded: Arc::new(expanded),
        }
    }

    pub fn inner(&self) -> &G {
        &self.graph
    }

    pub fn inner_mut(&mut self) -> &mut G {
        &mut self.graph
    }

    pub fn expanded(&self) -> &WeakMap<Thunk<G::Ctx>, bool> {
        &self.expanded
    }

    pub fn toggle(&mut self, thunk: &Thunk<G::Ctx>) {
        let mut expanded = (*self.expanded).clone();
        expanded[thunk] ^= true;
        self.expanded = Arc::new(expanded);
    }

    pub fn set_all(&mut self, value: bool) {
        let mut expanded = (*self.expanded).clone();
        expanded.values_mut().for_each(|x| *x = value);
        self.expanded = Arc::new(expanded);
    }
}

#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Eq(bound = ""),
    PartialEq(bound = ""),
    Hash(bound = ""),
    Debug(bound = "")
)]
pub struct CollapseEdge<G: Graph> {
    edge: Edge<G::Ctx>,
    #[derivative(PartialEq = "ignore", Hash = "ignore", Debug = "ignore")]
    expanded: Arc<WeakMap<Thunk<G::Ctx>, bool>>,
}

impl<G: Graph> CollapseEdge<G> {
    pub fn inner(&self) -> &Edge<G::Ctx> {
        &self.edge
    }

    pub fn into_inner(self) -> Edge<G::Ctx> {
        self.edge
    }
}

#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Eq(bound = ""),
    PartialEq(bound = ""),
    Hash(bound = ""),
    Debug(bound = "")
)]
pub struct CollapseOperation<G: Graph> {
    node: Node<G::Ctx>,
    #[derivative(PartialEq = "ignore", Hash = "ignore", Debug = "ignore")]
    expanded: Arc<WeakMap<Thunk<G::Ctx>, bool>>,
}

impl<G: Graph> CollapseOperation<G> {
    pub fn inner(&self) -> &Node<G::Ctx> {
        &self.node
    }

    pub fn into_inner(self) -> Node<G::Ctx> {
        self.node
    }
}

#[derive(Derivative)]
#[derivative(
    Clone(bound = ""),
    Eq(bound = ""),
    PartialEq(bound = ""),
    Hash(bound = ""),
    Debug(bound = "")
)]
pub struct CollapseThunk<G: Graph> {
    thunk: Thunk<G::Ctx>,
    #[derivative(PartialEq = "ignore", Hash = "ignore", Debug = "ignore")]
    expanded: Arc<WeakMap<Thunk<G::Ctx>, bool>>,
}

impl<G: Graph> CollapseThunk<G> {
    pub fn inner(&self) -> &Thunk<G::Ctx> {
        &self.thunk
    }

    pub fn into_inner(self) -> Thunk<G::Ctx> {
        self.thunk
    }
}

////////////////////////////////////////////////////////////////

pub type CollapseNode<G> = Node<CollapseGraph<G>>;

impl<G: Graph> CollapseNode<G> {
    fn new(node: Node<G::Ctx>, expanded: Arc<WeakMap<Thunk<G::Ctx>, bool>>) -> Self {
        match node {
            Node::Operation(op) => Node::Operation(CollapseOperation {
                node: Node::Operation(op),
                expanded,
            }),
            Node::Thunk(thunk) => {
                if expanded[&thunk] {
                    Node::Thunk(CollapseThunk { thunk, expanded })
                } else {
                    Node::Operation(CollapseOperation {
                        node: Node::Thunk(thunk),
                        expanded,
                    })
                }
            }
        }
    }

    pub fn into_inner(self) -> Node<G::Ctx> {
        match self {
            Node::Operation(op) => op.into_inner(),
            Node::Thunk(thunk) => Node::Thunk(thunk.into_inner()),
        }
    }
}

////////////////////////////////////////////////////////////////

impl<G: Graph> Ctx for CollapseGraph<G> {
    type Edge = CollapseEdge<G>;
    type Operation = CollapseOperation<G>;
    type Thunk = CollapseThunk<G>;
}

impl<G: Graph> Graph for CollapseGraph<G> {
    type Ctx = CollapseGraph<G>;

    fn free_graph_inputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.graph.free_graph_inputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn bound_graph_inputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(std::iter::empty())
    }

    fn graph_outputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.graph.graph_outputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn nodes(&self) -> Box<dyn DoubleEndedIterator<Item = Node<Self::Ctx>> + '_> {
        Box::new(
            self.graph
                .nodes()
                .map(|node| Node::new(node, self.expanded.clone())),
        )
    }

    fn graph_backlink(&self) -> Option<Thunk<Self::Ctx>> {
        None
    }
}

impl<G: Graph> EdgeLike for CollapseEdge<G> {
    type Ctx = CollapseGraph<G>;

    fn source(&self) -> Option<Node<Self::Ctx>> {
        self.edge
            .source()
            .map(|node| Node::new(node, self.expanded.clone()))
    }

    fn targets(&self) -> Box<dyn DoubleEndedIterator<Item = Option<Node<Self::Ctx>>> + '_> {
        Box::new(self.edge.targets().map(|target| {
            target.map(|mut node| {
                // If the node is transitively contained in a collapsed thunk, replace it with the thunk.
                if let Some(thunk) = find_ancestor(&node, |thunk| !self.expanded[thunk]) {
                    node = Node::Thunk(thunk);
                }
                Node::new(node, self.expanded.clone())
            })
        }))
    }
}

impl<G: Graph> NodeLike for CollapseOperation<G> {
    type Ctx = CollapseGraph<G>;

    fn inputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.node.inputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn outputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.node.outputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn backlink(&self) -> Option<Thunk<Self::Ctx>> {
        self.node.backlink().map(|thunk| CollapseThunk {
            thunk,
            expanded: self.expanded.clone(),
        })
    }

    fn number_of_inputs(&self) -> usize {
        self.node.number_of_inputs()
    }

    fn number_of_outputs(&self) -> usize {
        self.node.number_of_outputs()
    }
}

impl<G: Graph> Graph for CollapseThunk<G> {
    type Ctx = CollapseGraph<G>;

    fn free_graph_inputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.thunk.free_graph_inputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn bound_graph_inputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.thunk.bound_graph_inputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn graph_outputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.thunk.graph_outputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn nodes(&self) -> Box<dyn DoubleEndedIterator<Item = Node<Self::Ctx>> + '_> {
        Box::new(
            self.thunk
                .nodes()
                .map(|node| Node::new(node, self.expanded.clone())),
        )
    }

    fn graph_backlink(&self) -> Option<Thunk<Self::Ctx>> {
        Some(self.clone())
    }
}

impl<G: Graph> NodeLike for CollapseThunk<G> {
    type Ctx = CollapseGraph<G>;

    fn inputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.thunk.inputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn outputs(&self) -> Box<dyn DoubleEndedIterator<Item = Edge<Self::Ctx>> + '_> {
        Box::new(self.thunk.outputs().map(|edge| CollapseEdge {
            edge,
            expanded: self.expanded.clone(),
        }))
    }

    fn backlink(&self) -> Option<Thunk<Self::Ctx>> {
        self.thunk.backlink().map(|thunk| CollapseThunk {
            thunk,
            expanded: self.expanded.clone(),
        })
    }

    fn number_of_inputs(&self) -> usize {
        self.thunk.number_of_inputs()
    }

    fn number_of_outputs(&self) -> usize {
        self.thunk.number_of_outputs()
    }
}

impl<G: Graph + Codeable> Codeable for CollapseGraph<G> {
    type Code = Code<G>;

    fn code(&self) -> Self::Code {
        self.graph.code()
    }
}

impl<G: Graph> Codeable for CollapseEdge<G>
where
    Edge<G::Ctx>: Codeable,
{
    type Code = Code<Edge<G::Ctx>>;

    fn code(&self) -> Self::Code {
        self.edge.code()
    }
}

impl<G: Graph> Codeable for CollapseOperation<G>
where
    Operation<G::Ctx>: Codeable,
    Thunk<G::Ctx>: Codeable,
{
    type Code = Either<Code<Operation<G::Ctx>>, Code<Thunk<G::Ctx>>>;

    fn code(&self) -> Self::Code {
        match &self.node {
            Node::Operation(op) => Either::Left(op.code()),
            Node::Thunk(thunk) => Either::Right(thunk.code()),
        }
    }
}

impl<G: Graph> Codeable for CollapseThunk<G>
where
    Thunk<G::Ctx>: Codeable,
{
    type Code = Code<Thunk<G::Ctx>>;

    fn code(&self) -> Self::Code {
        self.thunk.code()
    }
}

impl<G: Graph> Matchable for CollapseEdge<G>
where
    Edge<G::Ctx>: Matchable,
{
    fn is_match(&self, query: &str) -> bool {
        self.edge.is_match(query)
    }
}

impl<G: Graph> Matchable for CollapseOperation<G>
where
    Operation<G::Ctx>: Matchable,
    Thunk<G::Ctx>: Matchable,
{
    fn is_match(&self, query: &str) -> bool {
        match &self.node {
            Node::Operation(op) => op.is_match(query),
            Node::Thunk(thunk) => thunk.is_match(query),
        }
    }
}

impl<G: Graph> Matchable for CollapseThunk<G>
where
    Thunk<G::Ctx>: Matchable,
{
    fn is_match(&self, query: &str) -> bool {
        self.thunk.is_match(query)
    }
}

impl<G: Graph> WithWeight for CollapseEdge<G> {
    type Weight = EdgeWeight<G::Ctx>;

    fn weight(&self) -> Self::Weight {
        self.edge.weight()
    }
}

impl<G: Graph> WithWeight for CollapseOperation<G> {
    type Weight = Either<OperationWeight<G::Ctx>, ThunkWeight<G::Ctx>>;

    fn weight(&self) -> Self::Weight {
        match &self.node {
            Node::Operation(op) => Either::Left(op.weight()),
            Node::Thunk(thunk) => Either::Right(thunk.weight()),
        }
    }
}

impl<G: Graph> WithWeight for CollapseThunk<G> {
    type Weight = ThunkWeight<G::Ctx>;

    fn weight(&self) -> Self::Weight {
        self.thunk.weight()
    }
}

/// Finds the ancestor of given node that satisfies the predicate.
fn find_ancestor<T: Ctx>(node: &Node<T>, f: impl Fn(&T::Thunk) -> bool) -> Option<T::Thunk> {
    let thunk = node.backlink()?;
    if f(&thunk) {
        Some(thunk)
    } else {
        find_ancestor::<T>(&Node::Thunk(thunk), f)
    }
}

impl<G: Graph> ExtensibleEdge for CollapseEdge<G>
where
    Edge<G::Ctx>: ExtensibleEdge,
{
    fn extend_source(&self) -> Option<Node<Self::Ctx>> {
        self.inner()
            .extend_source()
            .map(|node| Node::new(node, self.expanded.clone()))
    }

    fn extend_targets(&self) -> Box<dyn DoubleEndedIterator<Item = Node<Self::Ctx>> + '_> {
        Box::new(
            self.inner()
                .extend_targets()
                .map(|node| Node::new(node, self.expanded.clone())),
        )
    }
}