use std::{collections::HashSet, fmt::Display};

use eframe::egui;
use sd_core::{
    decompile::decompile,
    graph::{Name, Op, SyntaxHyperGraph},
    hypergraph::{subgraph::Free, Operation},
    language::{chil::Chil, spartan::Spartan, Expr, Language},
    prettyprinter::PrettyPrint,
};

use crate::{
    code_ui::code_ui,
    graph_ui::{GraphUi, GraphUiInternal},
    parser::UiLanguage,
};

pub(crate) enum Selection {
    Chil(SelectionInternal<Chil>),
    Spartan(SelectionInternal<Spartan>),
}

impl Selection {
    pub(crate) fn ui(&mut self, ctx: &egui::Context) {
        match self {
            Self::Chil(selection) => selection.ui(ctx),
            Self::Spartan(selection) => selection.ui(ctx),
        }
    }

    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Chil(selection) => &selection.name,
            Self::Spartan(selection) => &selection.name,
        }
    }

    pub(crate) fn displayed(&mut self) -> &mut bool {
        match self {
            Self::Chil(selection) => &mut selection.displayed,
            Self::Spartan(selection) => &mut selection.displayed,
        }
    }

    pub fn from_graph(graph_ui: &GraphUi, name: String, ctx: &egui::Context) -> Option<Self> {
        match graph_ui {
            GraphUi::Empty => None,
            GraphUi::Chil(graph_ui) => Some(Self::Chil(SelectionInternal::new(
                &graph_ui.current_selection,
                name,
                &graph_ui.hypergraph,
                ctx,
            ))),
            GraphUi::Spartan(graph_ui) => Some(Self::Spartan(SelectionInternal::new(
                &graph_ui.current_selection,
                name,
                &graph_ui.hypergraph,
                ctx,
            ))),
        }
    }
}

pub(crate) struct SelectionInternal<T: Language> {
    pub(crate) name: String,
    pub(crate) displayed: bool,
    code: String,
    graph_ui: GraphUiInternal<T>,
}

impl<T: 'static + Language> SelectionInternal<T> {
    pub(crate) fn new(
        selected_nodes: &HashSet<Operation<Op<T>, Name<T>>>,
        name: String,
        containing_graph: &SyntaxHyperGraph<T>,
        ctx: &egui::Context,
    ) -> Self
    where
        T::Op: Display,
        T::Var: Free,
        Expr<T>: PrettyPrint,
    {
        let normalised = containing_graph.normalise_selection(selected_nodes);
        let hypergraph = containing_graph.generate_subgraph(&normalised);

        let code = decompile(&hypergraph)
            .map_or_else(|err| format!("Error: {err:?}"), |expr| expr.to_pretty());

        let graph_ui = GraphUiInternal::from_graph(hypergraph, ctx);

        Self {
            code,
            name,
            displayed: true,
            graph_ui,
        }
    }

    pub(crate) fn ui(&mut self, ctx: &egui::Context)
    where
        T::Op: Display,
        T::Op: PrettyPrint,
        T::Var: PrettyPrint,
        T::Addr: Display,
        T::VarDef: PrettyPrint,
        Expr<T>: PrettyPrint,
    {
        egui::Window::new(self.name.clone())
            .open(&mut self.displayed)
            .show(ctx, |ui| {
                ui.columns(2, |columns| {
                    code_ui(
                        &mut columns[0],
                        &mut self.code.as_str(),
                        UiLanguage::Spartan,
                    );
                    self.graph_ui.ui(&mut columns[1]);
                });
            });
    }
}
