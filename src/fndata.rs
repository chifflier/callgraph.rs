use std::collections::{HashSet,HashMap};
use std::fs::File;

use rls_data::Id;


// Processed data about our crate. See comments on visitor::FnVisitor for more
// detail.
pub struct FnData {
    pub static_calls: HashSet<(Id, Id)>,
    // (caller def, callee def) c.f., FnVisitor::dynamic_calls.
    pub dynamic_calls: HashSet<(Id, Id)>,    
    pub functions: HashMap<Id, String>,

    pub crate_name: String
}

impl FnData {
    // Make a graphviz dot file.
    // Must be called after post_process.
    pub fn dot(&self) {
        let mut file = File::create(&format!("{}.dot", self.crate_name)).unwrap();
        rustc_graphviz::render(self, &mut file).unwrap();
    }

    // Dump collected and processed information to stdout.
    pub fn dump(&self) {
        println!("Found fns:");
        for (k, d) in self.functions.iter() {
            println!("{}:{}: {}", k.krate, k.index, d);
        }

        println!("\nFound calls:");
        for &(ref from, ref to) in self.static_calls.iter() {
            println!("{:?} -> {:?}", from, to);
            let from = &self.functions[from];
            let to = &self.functions[to];
            println!("{} -> {}", from, to);
        }

        println!("\nFound potential calls:");
        for &(ref from, ref to) in self.dynamic_calls.iter() {
            let from = &self.functions[from];
            let to = &self.functions[to];
            println!("{} -> {}", from, to);
        }
    }

}
