extern crate grdeval;

use std::env;
use std::process;

use grdeval::Config;

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = Config::new(&args).unwrap_or_else(|err| {
        println!("error: {}", err);
        process::exit(1);
    });

    if let Err(e) = grdeval::run(config) {
        println!("app error: {}", e);
        process::exit(1);
    }
}
