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

// Collection types.
pub use collections::HashSet;
pub use collections::BTreeSet;
pub use collections::BTreeMap;
pub use collections::VecDeque;
pub use collections::BinaryHeap;

// Error types.
pub use io::Error;
pub use io::ErrorKind;

// Text input and output.
pub use io::read_line;
pub use io::read_file;
pub use io::write_file;

// Process helpers.
pub use process::args;

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

// Time.
pub use time::DateTime;
pub use time::now_ms;
pub use time::monotonic_ns;
pub use time::sleep_ms;

// Environment.
pub use env::var;
pub use env::vars;
