mod report;

use report::build_report;
use util_a::MODULUS;
use util_a::checksum as util_a_checksum;
use util_b::checksum as util_b_checksum;

fn main() {
    let seed = 13;
    let report = build_report(seed);
    let mut acc = util_a_checksum(seed);
    acc = (acc * 31 + util_b_checksum(seed)) % MODULUS;
    acc = (acc * 31 + report.checksum) % MODULUS;
    acc = (acc * 31 + report.label.len() as i64) % MODULUS;
    acc = (acc * 31 + report.metrics) % MODULUS;
    if report.is_valid() {
        acc = (acc * 31 + 1) % MODULUS;
    } else {
        acc = (acc * 31 + 2) % MODULUS;
    }
    println!("checksum={}", acc);
}
