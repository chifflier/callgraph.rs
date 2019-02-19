#![feature(rustc_private)]

extern crate rls_data;
extern crate graphviz as rustc_graphviz;
extern crate rustc;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_save_analysis;
extern crate syntax;
extern crate syntax_pos;

use rustc::session::Session;
use rustc::session::config::Input;
use rustc_driver::{driver, CompilerCalls, Compilation, getopts::Matches};
use rustc_save_analysis::{SaveContext, SaveHandler};
use rustc_save_analysis as save;
use syntax::{ast,visit};

use std::env;
use std::process::Command;

// Where all the work is done
mod visitor;

mod fndata;
mod graphviz;

pub const SKIP_UNCONNECTED_FNS: bool = false;

// Coordinates the compiler, doesn't need any state for callgraphs.
struct CallGraphCalls;

// A bunch of callbacks from the compiler. We don't do much, mostly accept the
// default implementations.
impl<'a> CompilerCalls<'a> for CallGraphCalls {
    fn build_controller(self: Box<Self>, _: &Session, _:&Matches) -> driver::CompileController<'a> {
        // Mostly, we want to copy what rustc does.
        let mut control = driver::CompileController::basic();
        // Keep expanded_crate after expand step
        control.keep_ast = true;
        // But we can stop after analysis, we don't need to generate code.
        control.after_analysis.stop = Compilation::Stop;
        control.after_analysis.callback = Box::new(move |state| {
            // eprintln!("after_analysis");
            // eprintln!("  krate: {}", if let Some(_) = state.krate {"OK"} else {"FAIL"});
            // eprintln!("  expanded_crate: {}", state.expanded_crate.map_or_else(|| "FAIL", |_| "OK"));
            // eprintln!("  HIR crate: {}", state.hir_crate.map_or_else(|| "FAIL", |_| "OK"));
            // eprintln!("  tcx: {}", state.tcx.map_or_else(|| "FAIL", |_| "OK"));
            save::process_crate(
                state.tcx.expect("missing tcx"),
                state.expanded_crate.expect("missing crate"),
                state.crate_name.expect("missing crate name"),
                state.input,
                None,
                FnSaveHandler
            );
        });

        control
    }
}

struct FnSaveHandler;

impl SaveHandler for FnSaveHandler {
    fn save<'l, 'tcx>(
        &mut self,
        save_ctxt: SaveContext<'l, 'tcx>,
        krate: &ast::Crate,
        crate_name: &str,
        _input: &'l Input
    )
    {
        // eprintln!("SaveHandler");
        // eprintln!("Krate: {:#?}", krate);
        let mut visitor = visitor::FnVisitor::new(save_ctxt);
        // This actually does the walking.
        visit::walk_crate(&mut visitor, krate);
        // // When we're done, process the info we collected.
        let data = visitor.post_process(crate_name);
        // // Then produce output.
        data.dump();
        data.dot();
    }
}

// args are the arguments passed on the command line, generally passed through
// to the compiler.
pub fn run(args: Vec<String>) {
    let mut args = args.clone();

    // Create a data structure to control compilation.
    let calls = Box::new(CallGraphCalls);

    let sysroot = current_sysroot()
        .expect("need to specify SYSROOT env var or use rustup or multirust");

    args.push("--sysroot".to_owned());
    args.push(sysroot);

    // Run the compiler!
    syntax::with_globals(|| {
        rustc_driver::run_compiler(&args, calls, None, None);
    });
}

fn current_sysroot() -> Option<String> {
    let home = env::var("RUSTUP_HOME").or_else(|_| env::var("MULTIRUST_HOME"));
    let toolchain = env::var("RUSTUP_TOOLCHAIN").or_else(|_| env::var("MULTIRUST_TOOLCHAIN"));
    if let (Ok(home), Ok(toolchain)) = (home, toolchain) {
        Some(format!("{}/toolchains/{}", home, toolchain))
    } else {
        let rustc_exe = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_owned());
        env::var("SYSROOT").ok().or_else(|| {
            Command::new(rustc_exe)
                .arg("--print")
                .arg("sysroot")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_owned())
        })
    }
}
