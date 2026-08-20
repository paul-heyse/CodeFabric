//! Single private seam around Pyrefly's explicitly unstable Rust API.

use pyrefly::library::library::library::library::default_config_finder;
use pyrefly::query::Query;
use pyrefly_util::thread_pool::ThreadCount;

pub(crate) fn query_surface_smoke() -> usize {
    let query = Query::new(default_config_finder(None), ThreadCount::Inline);
    size_of_val(&query)
}
