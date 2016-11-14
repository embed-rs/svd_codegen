extern crate clap;
extern crate svd_codegen;
extern crate svd_parser as svd;

use std::ascii::AsciiExt;
use std::fs::File;
use std::io::Read;

use clap::{App, Arg};

fn main() {
    let matches = App::new("svd_codegen")
        .about("Generate Rust register maps (`struct`s) from SVD files")
        .arg(Arg::with_name("input")
            .help("Input SVD file")
            .required(true)
            .short("i")
            .takes_value(true)
            .value_name("FILE"))
        .arg(Arg::with_name("peripheral")
            .help("Pattern used to select a single peripheral")
            .value_name("PATTERN"))
        .version(concat!(env!("CARGO_PKG_VERSION"),
                         include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))))
        .get_matches();

    let xml = &mut String::new();
    File::open(matches.value_of("input").unwrap())
        .unwrap()
        .read_to_string(xml)
        .unwrap();

    let mut d = svd::parse(xml);
    match matches.value_of("peripheral") {
        None => {
            for peripheral in &d.peripherals {
                println!("const {}: usize = 0x{:08x};",
                         peripheral.name,
                         peripheral.base_address);
            }
        }
        Some(pattern) => {
            for peripheral in &mut d.peripherals {
                if peripheral.name.to_ascii_lowercase().contains(&pattern) {
                    println!("{}",
                             svd_codegen::gen_peripheral(peripheral, &d.defaults)
                                 .iter()
                                 .map(|i| i.to_string())
                                 .collect::<Vec<_>>()
                                 .join("\n\n"));

                    break;
                }
            }
        }
    }
}
