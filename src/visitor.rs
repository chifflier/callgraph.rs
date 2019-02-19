use rls_data::{DefKind,RefKind};
use rustc_save_analysis::{self, SaveContext};
use rustc_save_analysis as save;
// use rustc::hir::def_id::{DefId,LOCAL_CRATE};

use syntax::ast;
use syntax::visit;

// use syntax::source_map::Span;

use std::collections::{HashSet,HashMap};

use crate::fndata::FnData;

pub struct FnVisitor<'l, 'tcx: 'l> {
    // Used by the save-analysis API.
    save_cx: SaveContext<'l, 'tcx>,

    // Track statically dispatched function calls.
    static_calls: HashSet<(rls_data::Id, rls_data::Id)>,
    // (caller def, callee decl).
    dynamic_calls: HashSet<(rls_data::Id, rls_data::Id)>,
    // Track function definitions.
    functions: HashMap<rls_data::Id, String>,
    // Track method declarations.
    method_decls: HashMap<rls_data::Id, String>,
    // Maps a method decl to its implementing methods.
    method_impls: HashMap<rls_data::Id, Vec<rls_data::Id>>,

    // Which function we're calling from, we'll update this as we walk the AST.
    cur_fn: Option<rls_data::Id>,
}

// `this.cur_fn.is_some()` or returns.
macro_rules! ensure_cur_fn {($this: expr, $span: expr) => {
    if $this.cur_fn.is_none() {
        println!("WARNING: call at {:?} without known current function",
                 $span);
        return;
    }
}}

// Backup self.cur_fn, set cur_fn to id, continue to walk the AST by executing
// $walk, then restore self.cur_fn.
macro_rules! push_walk_pop {($this: expr, $id: expr, $walk: expr) => {{
    let prev_fn = $this.cur_fn;
    $this.cur_fn = Some($id);
    $walk;
    $this.cur_fn = prev_fn;
}}}

// Return if we're in generated code.
// Note: rustc_save_analysis::generated_code is priavte, so include the equivalent code
// See https://doc.rust-lang.org/nightly/nightly-rustc/rustc_save_analysis/fn.generated_code.html
macro_rules! skip_generated_code {($span: expr) => {
    if $span.ctxt() != syntax_pos::NO_EXPANSION || $span.is_dummy() {
        return;
    }
}}

// True if the def_id refers to an item in the current crate.
fn is_local(id: rls_data::Id) -> bool {
    id.krate == 0
}







impl<'l, 'tcx: 'l> FnVisitor<'l, 'tcx> {
    pub fn new(save_cx: SaveContext<'l, 'tcx>) -> FnVisitor<'l, 'tcx> {
        FnVisitor{
            save_cx,
            static_calls: HashSet::new(),
            dynamic_calls: HashSet::new(),
            functions: HashMap::new(),
            method_decls: HashMap::new(),
            method_impls: HashMap::new(),
            cur_fn: None
        }
    }

    pub fn post_process(self, crate_name: &str) -> FnData {
        let mut processed_calls = HashSet::new();
        let mut processed_fns = HashMap::with_capacity(self.functions.len());

        for &(ref from, ref to) in self.dynamic_calls.iter() {
            for to in self.method_impls[to].iter() {
                processed_calls.insert((*from, *to));
                self.append_fn(&mut processed_fns, *from);
                self.append_fn(&mut processed_fns, *to);
            }
        }
        if super::SKIP_UNCONNECTED_FNS {
            for &(ref from, ref to) in self.static_calls.iter() {
                self.append_fn(&mut processed_fns, *from);
                self.append_fn(&mut processed_fns, *to);
            }
        }
        FnData {
            static_calls: self.static_calls,
            dynamic_calls: processed_calls,
            functions: if super::SKIP_UNCONNECTED_FNS {
                    processed_fns
                } else {
                    self.functions
                },
            crate_name: crate_name.to_string()
        }
    }

    // If we are skipping unconnected functions, then keep track of which
    // functions are connected.
    fn append_fn(&self, map: &mut HashMap<rls_data::Id, String>, id: rls_data::Id) {
        if !super::SKIP_UNCONNECTED_FNS {
            return;
        }

        if map.contains_key(&id) {
            return;
        }
        eprintln!("id: {:?}", id);
        debug_assert!(self.functions.contains_key(&id));

        map.insert(id, self.functions[&id].clone());
    }

    // Record that def implements decl.
    fn append_method_impl(&mut self, decl: rls_data::Id, def: rls_data::Id) {
        if !self.method_impls.contains_key(&decl) {
            self.method_impls.insert(decl, vec![]);
        }

        self.method_impls.get_mut(&decl).unwrap().push(def);
    }

}

// See https://doc.rust-lang.org/nightly/nightly-rustc/syntax/visit/trait.Visitor.html
//
impl<'v, 'l, 'tcx: 'l> visit::Visitor<'v> for FnVisitor<'l, 'tcx> {
    // Visit a path - the path could point to a function or method.
    fn visit_path(&mut self, path: &'v ast::Path, id: ast::NodeId) {
        eprintln!("visit_path id={:?}", id);
        // eprintln!("path: {:?}", path);
        // eprintln!("id: {:?}", id);
        skip_generated_code!(path.span);

        let data = self.save_cx.get_path_data(id, path);
        eprintln!("    get_path_data {:?}", data);
        // eprintln!("data: {:?}", data);
        if let Some(ref rfd) = data {
            eprintln!("    rfd: {:?}", rfd);
            if rfd.kind == RefKind::Function {
                if is_local(rfd.ref_id) {
                    let to = rfd.ref_id;
                    ensure_cur_fn!(self, rfd.span);
                    eprintln!("***  {:?} -> {:?} ***", self.cur_fn.unwrap(), to);
                    eprintln!("  path: {:?}", path);
                    eprintln!("  span: {:?}", path.span);
                    self.static_calls.insert((self.cur_fn.unwrap(), to));
                } else {
                    eprintln!("NOT local");
                }
            }
            // if let Some(save::Data::MethodCallData(ref mrd)) = data {
            //     self.record_method_call(mrd);
            // }
        }

        // Continue walking the AST.
        visit::walk_path(self, path)
    }

    // fn visit_fn(&mut self, fk: visit::FnKind<'v>, fd: &'v ast::FnDecl, s: Span, _: ast::NodeId) {
    //     eprintln!("visit_fn {:?}", fd);
    //     visit::walk_fn(self, fk, fd, s)
    // }

    // Visit an expression
    fn visit_expr(&mut self, ex: &'v ast::Expr) {
        skip_generated_code!(ex.span);

        visit::walk_expr(self, ex);

        eprintln!("visit_expr: {:?}", ex);
        eprintln!("  node: {:?}", ex.node);

        eprintln!("  get_expr_data: {:?}", self.save_cx.get_expr_data(ex));
        if let ast::ExprKind::Call(ref _f, ref _args) = ex.node {
            eprintln!("    Call");
            eprintln!("    node: {:?}", ex.node);
            eprintln!("    f:    {:?}", _f);
            eprintln!("        get_expr_data: {:?}", self.save_cx.get_expr_data(_f));
            eprintln!("    args: {:?}", _args);
        }
        if let ast::ExprKind::MethodCall(ref _seg, ref _args) = ex.node {
            eprintln!("    MethodCall");
            eprintln!("    node: {:?}", ex.node);
            eprintln!("    seg:  {:?}", _seg);
            eprintln!("        get_path_segment_data: {:?}", self.save_cx.get_path_segment_data(_seg));
        }

        // Skip everything except method calls. (We shouldn't have to do this, but
        // calling get_expr_data on an expression it doesn't know about will panic).
        // if let ast::ExprKind::MethodCall(..) = ex.node {} else {
        //     return;
        // }

        let data = self.save_cx.get_expr_data(ex);
        dbg!(&data);
        if data.is_some() { unimplemented!(); }
        // if let Some(save::Data::MethodCallData(ref mrd)) = data {
        //     self.record_method_call(mrd);
        // }
    }

    fn visit_item(&mut self, item: &'v ast::Item) {
        eprintln!("visit_item ident={:?}", item.ident);
        skip_generated_code!(item.span);
        if let ast::ItemKind::Fn(..) = item.node {
            // eprintln!("Got function item for {}", item.ident.to_string());
            let data = self.save_cx.get_item_data(item);
            // eprintln!("data: {:?}", data);
            if let Some(ref d) = data {
                // eprintln!("d: {:?}", d);
                if let save::Data::DefData(ref fd) = d {
                    if fd.kind == DefKind::Function {
                        // dbg!(&fd);
                        eprintln!("***  defining function {}: {:?} ***", fd.qualname, fd.id);
                        self.functions.insert(fd.id, fd.qualname.clone());
                        // eprintln!("***  {} -> {} ***", from, to);
                        push_walk_pop!(self, fd.id, visit::walk_item(self, item));
                        return;
                    }
                }
                if let save::Data::RelationData(ref rel, ref im) = d {
                    eprintln!("RelationData");
                }
                if let save::Data::RefData(ref fd) = d {
                    eprintln!("RefData");
                }
            }
        }
        visit::walk_item(self, item)
    }

    fn visit_trait_item(&mut self, ti: &'v ast::TraitItem) {
        skip_generated_code!(ti.span);
        // Note to self: it is kinda sucky we have to examine the AST before
        // asking for data here.
        match ti.node {
            // A method declaration.
            ast::TraitItemKind::Method(_, None) => {
                let fd = self.save_cx.get_method_data(ti.id, ti.ident, ti.span).expect("get_method_data");
                self.method_decls.insert(fd.id, fd.qualname);
                self.method_impls.insert(fd.id, vec![]);
            }
            // A default method. This declares a trait method and provides an
            // implementation.
            ast::TraitItemKind::Method(_, Some(_)) => {
                let fd = self.save_cx.get_method_data(ti.id, ti.ident, ti.span).expect("get_method_data");
                // Record, a declaration, a definintion, and a reflexive implementation.
                eprintln!("***  defining method {}: {:?} ***", fd.qualname, fd.id);
                self.method_decls.insert(fd.id, fd.qualname.clone());
                self.functions.insert(fd.id, fd.qualname);
                self.append_method_impl(fd.id, fd.id);
                push_walk_pop!(self, fd.id, visit::walk_trait_item(self, ti));

                return;
            }
            _ => {}
        }

        visit::walk_trait_item(self, ti)
    }

    fn visit_impl_item(&mut self, ii: &'v ast::ImplItem) {
        skip_generated_code!(ii.span);

        if let ast::ImplItemKind::Method(..) = ii.node {
            let fd = self.save_cx.get_method_data(ii.id, ii.ident, ii.span).expect("get_method_data");
            eprintln!("visit_impl_item: {:?} {}", fd.id, fd.qualname);
            eprintln!("  ii.id: {:?}", ii.id);
            eprintln!("  fd: {:?}", fd);
            // Record the method's existence.
            eprintln!("***  defining method {}: {:?} ***", fd.qualname, fd.id);
            self.functions.insert(fd.id, fd.qualname);
            if let Some(decl) = fd.decl_id {
                if is_local(decl) {
                    eprintln!("is_local");
                    // If we're implementing a method in the local crate, record
                    // the implementation of the decl.
                    self.append_method_impl(decl, fd.id);
                }
            }

            push_walk_pop!(self, fd.id, visit::walk_impl_item(self, ii));

            return;
        }

        visit::walk_impl_item(self, ii)
    }


}
