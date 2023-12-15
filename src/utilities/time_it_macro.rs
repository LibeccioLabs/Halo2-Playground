/// A macro to take the execution time of some code.
///
/// ## Usage example
///
/// ```ignore
/// time_it!(
///     "adding 100000 numbers takes {:?} time",
///     {
///         let _sum = (0..100000).fold(0_u64, |sum, n| sum + n);
///     }
/// );
/// ```
#[macro_export]
macro_rules! time_it {
    ($print_message: literal, $($thing_to_time: tt)*) => {{
        let t0 = std::time::Instant::now();

        let ret = {
            $($thing_to_time)*
        };

        let t1 = std::time::Instant::now();
        let time = t1 - t0;
        println!($print_message, time);

        ret
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn time_it_test() {
        time_it!("adding 100000 numbers takes {:?} time", {
            let _sum = (0..100000).fold(0_u64, |sum, n| sum + n);
        });
    }
}
