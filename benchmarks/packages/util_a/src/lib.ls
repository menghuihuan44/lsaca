// util_a: shared numeric, text and digest helpers.

pub const MODULUS: i64 = 1000000007;

mod numbers;
mod text;
mod digest;

pub use numbers::{gcd, lcm, is_prime, prime_count, primes_upto, digit_total, reverse_digits, collatz_steps, power_mod, triangular, sum_squares, factorial_mod, gcd_of_slice, lcm_of_slice, next_square, Sign, sign_of, sign_code, Span};
pub use text::{word_lengths, list_words, word_total, repeat, shout, whisper, csv_total, csv_count, digit_total_text, count_digits, decorate, join_words, first_letters, longest_word};
pub use digest::{Mixer, normalize, mix, hash_text, fold_words};

use numbers::checksum as numbers_checksum;
use text::checksum as text_checksum;
use digest::checksum as digest_checksum;

pub fn checksum(seed: i64) -> i64 {
    let mut acc = seed % MODULUS;
    acc = (acc * 31 + numbers_checksum(seed)) % MODULUS;
    acc = (acc * 31 + text_checksum(seed)) % MODULUS;
    acc = (acc * 31 + digest_checksum(seed)) % MODULUS;
    return acc;
}
