use indexmap::IndexSet;

use crate::language::{Expr, GetVar, Language, Thunk, Value};

impl<T: Language> Expr<T> {
    pub(crate) fn free_vars(&self) -> IndexSet<T::Var> {
        let mut vars: IndexSet<T::Var> = IndexSet::new();

        for bind in &self.binds {
            bind.value.free_vars(&mut vars);
        }

        for value in &self.values {
            value.free_vars(&mut vars);
        }

        for def in self.binds.iter().flat_map(|bind| &bind.defs) {
            vars.remove(def.var());
        }

        vars
    }
}

impl<T: Language> Value<T> {
    pub(crate) fn free_vars(&self, vars: &mut IndexSet<T::Var>) {
        match self {
            Value::Variable(v) => {
                vars.insert(v.clone());
            }
            Value::Thunk(thunk) => {
                thunk.free_vars(vars);
            }
            Value::Op { args, .. } => {
                for arg in args {
                    arg.free_vars(vars);
                }
            }
        }
    }
}

impl<T: Language> Thunk<T> {
    pub(crate) fn free_vars(&self, vars: &mut IndexSet<T::Var>) {
        let body_vars = self.body.free_vars();
        let arg_set: IndexSet<T::Var> = self.args.iter().map(GetVar::var).cloned().collect();
        vars.extend(body_vars.difference(&arg_set).cloned());
    }
}
