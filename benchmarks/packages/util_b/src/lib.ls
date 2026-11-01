// util_b: ordered counters, geometry and aggregates on top of util_a.

pub const MODULUS: i64 = 1000000007;

mod table;
mod geom;
mod stats;

pub use table::{word_count, total_occurrences, entry_signature, Counter, Entry};
pub use geom::{Point, Rect, abs_value, distance_squared, distance_manhattan};
pub use stats::{Stats, total, every_other_sum};

use table::checksum as table_checksum;
use geom::checksum as geom_checksum;
use stats::checksum as stats_checksum;

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    acc = (acc * 31 + table_checksum(seed)) % MODULUS;
    acc = (acc * 31 + geom_checksum(seed)) % MODULUS;
    acc = (acc * 31 + stats_checksum(seed)) % MODULUS;
    return acc;
}
