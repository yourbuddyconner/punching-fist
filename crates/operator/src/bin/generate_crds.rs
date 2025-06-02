use punching_fist_operator::crd::{Source, Workflow, Sink};
use kube::CustomResourceExt;

fn main() {
    // Generate Source CRD
    println!("---");
    println!("# Source CRD");
    println!("{}", serde_yaml::to_string(&Source::crd()).unwrap());
    
    println!("---");
    println!("# Workflow CRD");
    println!("{}", serde_yaml::to_string(&Workflow::crd()).unwrap());
    
    println!("---");
    println!("# Sink CRD");
    println!("{}", serde_yaml::to_string(&Sink::crd()).unwrap());
} 