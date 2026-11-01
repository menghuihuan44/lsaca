// report: builds the validation report for the compile_app benchmark package.

use util_a::decorate;
use util_a::hash_text;
use util_a::list_words;
use util_a::shout;
use util_a::Span;
use util_b::distance_squared;
use util_b::entry_signature;
use util_b::word_count;
use util_b::Point;
use util_b::Rect;
use util_b::Stats;

pub const MODULUS: i64 = 1000000007;

pub enum Format {
    Text,
    Terse,
}

pub struct Report {
    pub checksum: i64,
    pub metrics: i64,
    pub label: String,
}

impl Report {
    pub fn new(checksum: i64, metrics: i64, label: String) -> Report {
        return Report { checksum: checksum, metrics: metrics, label: label };
    }

    pub fn is_valid(&self) -> bool {
        if self.checksum < 0 {
            return false;
        }
        return self.metrics > 0;
    }

    pub fn label_checksum(&self) -> i64 {
        return hash_text(&self.label);
    }
}

pub fn format_name(kind: Format) -> String {
    match kind {
        Format::Text => {
            return String::from("text");
        }
        Format::Terse => {
            return String::from("terse");
        }
    }
}

pub fn build_report(seed: i64) -> Report {
    let mut acc = seed % MODULUS;
    let sentence = String::from("alpha beta gamma alpha delta beta alpha epsilon");
    let words = list_words(&sentence);
    let counter = word_count(words, 10);
    acc = (acc * 31 + counter.distinct()) % MODULUS;
    acc = (acc * 31 + counter.checksum()) % MODULUS;
    let entries = counter.entries();
    let mut stats = Stats::new();
    for entry in entries {
        let signature = entry_signature(&entry);
        acc = (acc * 31 + signature) % MODULUS;
        stats.observe(entry.count);
    }
    acc = (acc * 31 + stats.sum) % MODULUS;
    acc = (acc * 31 + stats.max) % MODULUS;
    let origin = Point::new(1, 2);
    let target = Point::new(6, 7);
    acc = (acc * 31 + distance_squared(&origin, &target)) % MODULUS;
    let frame = Rect::new(0, 0, 8, 8);
    if frame.contains(&target) {
        acc = (acc * 31 + 1) % MODULUS;
    } else {
        acc = (acc * 31 + 2) % MODULUS;
    }
    let base = String::from("report");
    let loud = shout(&base);
    let label = decorate(&loud, seed);
    acc = (acc * 31 + label.len()) % MODULUS;
    let span = Span::new(seed, 5);
    acc = (acc * 31 + span.checksum()) % MODULUS;
    let kind = Format::Text;
    let name = format_name(kind);
    acc = (acc * 31 + name.len()) % MODULUS;
    let metrics = stats.count * 10 + stats.range();
    return Report::new(acc, metrics, label);
}
