use rand::{rngs::StdRng, Rng, SeedableRng};
use std::cell::RefCell;

/*
 *  I didn't want to use GUIDs for the unique ID.  We need an ID that can be quickly generated, is unique,
 *  and works if multiple threads are creating documents.  I'm using Rand() seeded per thread.
 */

//macro for get_id
thread_local! {
    static RNG: RefCell<StdRng> = RefCell::new(StdRng::from_entropy());
}
pub fn get_id() -> String {
    format!("unique_id{}", RNG.with(|rng| rng.borrow_mut().gen::<u64>()))
}

#[macro_export]
macro_rules! log_return_err {
    ( $e:expr ) => {{
        log::error!("\t{}\n {:#?}", $e, $e);
        return Err($e);
    }};
}
