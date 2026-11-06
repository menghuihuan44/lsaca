// lsaca standard library.
//
// This is the crate root: it declares the modules of the standard library and
// re-exports the items that user programs see without qualification. The
// standard library is regular lsaca code except for `extern fn` intrinsics,
// which the compiler expands inline; only this crate may declare them.

pub mod core;
pub mod vec;
pub mod string;
pub mod char;
pub mod iter;
pub mod fmt;
pub mod math;
pub mod hash_map;
pub mod collections;
pub mod hash_set;
pub mod io;
pub mod fs;
pub mod net;
pub mod time;
pub mod env;
pub mod error;
pub mod process;

// Iterator trait and the hidden iterator adapters used by iterator sugar.
pub use iter::Iterator;
pub use iter::__lsaca_VecIter;
pub use iter::__lsaca_MapIter;
pub use iter::__lsaca_FilterIter;
pub use iter::__lsaca_TakeIter;
pub use iter::__lsaca_SkipIter;
pub use iter::__lsaca_EnumerateIter;
pub use iter::__lsaca_vec_iter;
pub use iter::__lsaca_it_map;
pub use iter::__lsaca_it_filter;
pub use iter::__lsaca_it_take;
pub use iter::__lsaca_it_skip;
pub use iter::__lsaca_it_enumerate;
pub use iter::__lsaca_it_collect_vec;
pub use iter::__lsaca_it_count;
pub use iter::__lsaca_it_fold;
pub use iter::__lsaca_it_sum_i64;
pub use iter::__lsaca_it_sum_f64;
pub use iter::__lsaca_it_any;
pub use iter::__lsaca_it_all;
pub use iter::__lsaca_it_find;

// v2.1 iterator adapters: hidden types and constructor helpers resolved by
// the iterator sugar in the standard library crate root.
pub use iter::__lsaca_ZipIter;
pub use iter::__lsaca_ChainIter;
pub use iter::__lsaca_FilterMapIter;
pub use iter::__lsaca_TakeWhileIter;
pub use iter::__lsaca_SkipWhileIter;
pub use iter::__lsaca_StepByIter;
pub use iter::__lsaca_InspectIter;
pub use iter::__lsaca_it_zip;
pub use iter::__lsaca_it_chain;
pub use iter::__lsaca_it_filter_map;
pub use iter::__lsaca_it_take_while;
pub use iter::__lsaca_it_skip_while;
pub use iter::__lsaca_it_step_by;
pub use iter::__lsaca_it_inspect;
pub use iter::__lsaca_it_for_each;
pub use iter::__lsaca_it_position;
pub use iter::__lsaca_it_last;
pub use iter::__lsaca_it_nth;
pub use iter::__lsaca_it_reduce;
pub use iter::__lsaca_it_sum_i32;
pub use iter::__lsaca_it_sum_usize;
pub use iter::__lsaca_it_product_i32;
pub use iter::__lsaca_it_product_i64;
pub use iter::__lsaca_it_product_usize;
pub use iter::__lsaca_it_product_f64;

// Text parsing helpers used by the generic `parse::<T>()` sugar and by the
// numeric parse methods on String and str.
pub use string::__lsaca_parse_i8;
pub use string::__lsaca_parse_i16;
pub use string::__lsaca_parse_i32;
pub use string::__lsaca_parse_i64;
pub use string::__lsaca_parse_isize;
pub use string::__lsaca_parse_u8;
pub use string::__lsaca_parse_u16;
pub use string::__lsaca_parse_u32;
pub use string::__lsaca_parse_u64;
pub use string::__lsaca_parse_usize;
pub use string::__lsaca_parse_f32;
pub use string::__lsaca_parse_f64;

// Collection types.
pub use collections::HashSet;
pub use collections::BTreeSet;
pub use collections::BTreeMap;
pub use collections::VecDeque;
pub use collections::BinaryHeap;

// Error types.
pub use io::Error;
pub use io::ErrorKind;
pub use core::ParseIntError;
pub use core::ParseFloatError;

// Text input and output.
pub use io::read_line;
pub use io::read_file;
pub use io::write_file;

// Process helpers.
pub use process::args;
pub use process::exit;

// Numeric helper functions.
pub use math::sqrt;
pub use math::floor;
pub use math::ceil;
pub use math::round;
pub use math::sin;
pub use math::cos;
pub use math::tan;
pub use math::ln;
pub use math::log10;
pub use math::exp;
pub use math::pow;

// Numeric constants (f64, exact Rust values; since 2.1.0).
pub use math::PI;
pub use math::E;
pub use math::TAU;
pub use math::SQRT_2;
pub use math::FRAC_1_SQRT_2;
pub use math::FRAC_1_PI;
pub use math::FRAC_2_PI;
pub use math::FRAC_2_SQRT_PI;
pub use math::LN_2;
pub use math::LN_10;
pub use math::LOG2_E;
pub use math::LOG10_E;
pub use math::PHI;
pub use math::FRAC_PI_2;
pub use math::FRAC_PI_3;
pub use math::FRAC_PI_4;

// Handles and standard streams.
pub use io::File;
pub use io::stdin;
pub use io::stdout;
pub use io::stderr;

// File system functions.
pub use fs::read_to_string;
pub use fs::write;
pub use fs::read;
pub use fs::append;
pub use fs::copy;
pub use fs::metadata;
pub use fs::Metadata;
pub use fs::FileType;
pub use fs::read_dir;
pub use fs::exists;
pub use fs::is_file;
pub use fs::is_dir;
pub use fs::create_dir;
pub use fs::create_dir_all;
pub use fs::remove_file;
pub use fs::remove_dir;
pub use fs::remove_dir_all;
pub use fs::rename;
pub use fs::canonicalize;
pub use fs::current_dir;
pub use fs::set_current_dir;
pub use fs::temp_dir;

// Networking.
pub use net::TcpListener;
pub use net::TcpStream;
pub use net::Shutdown;

// Time (the Duration/Instant/SystemTime types are new in 2.1.0; the
// millisecond helpers are deprecated values kept for compatibility).
pub use time::Duration;
pub use time::Instant;
pub use time::SystemTime;
pub use time::unix_epoch;
pub use time::sleep;
pub use time::DateTime;
pub use time::now_ms;
pub use time::monotonic_ns;
pub use time::sleep_ms;

// Environment.
pub use env::VarError;
pub use env::var;
pub use env::vars;
