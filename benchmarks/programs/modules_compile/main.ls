// modules_compile: a multi-module program used as a compile-time benchmark.
//
// The crate root declares eight external modules. Each module is a separate
// file with its own public items and a pub fn checksum(seed: i64) -> i64 that
// folds a deterministic summary of the module into the running accumulator.
// The root imports the eight checksum functions, folds them in declaration
// order and prints a single checksum line.

mod arithmetic;
mod vectors;
mod strings;
mod records;
mod generics;
mod maps;
mod traits;
mod pipeline;

use arithmetic::checksum as arithmetic_checksum;
use vectors::checksum as vectors_checksum;
use strings::checksum as strings_checksum;
use records::checksum as records_checksum;
use generics::checksum as generics_checksum;
use maps::checksum as maps_checksum;
use traits::checksum as traits_checksum;
use pipeline::checksum as pipeline_checksum;

const MODULUS: i64 = 1000000007;

fn main() {
    let seed: i64 = 7;
    let mut acc: i64 = seed % MODULUS;
    acc = (acc * 31 + arithmetic_checksum(seed)) % MODULUS;
    acc = (acc * 31 + vectors_checksum(seed)) % MODULUS;
    acc = (acc * 31 + strings_checksum(seed)) % MODULUS;
    acc = (acc * 31 + records_checksum(seed)) % MODULUS;
    acc = (acc * 31 + generics_checksum(seed)) % MODULUS;
    acc = (acc * 31 + maps_checksum(seed)) % MODULUS;
    acc = (acc * 31 + traits_checksum(seed)) % MODULUS;
    acc = (acc * 31 + pipeline_checksum(seed)) % MODULUS;
    println!("checksum={}", acc);
}
