/// This module defines a utility struct to iterate over a sequence of
/// `(div: usize, mod: usize)` values such that, given a non-zero `MOD: usize`
/// value, it holds that for each new item the quantity `div * MOD + mod`
/// increments by one, and such that, for every item, `0 <= mod < MOD`.
mod div_mod_counter;
pub use div_mod_counter::DivModCounter;

/// This module defines a utility trait that allows to easily assign arrays
/// and grids of values to columns in a circuit.
mod region_sequence_assignment;
pub use region_sequence_assignment::RegionSequenceAssignment;

/// This module implements an iterator `FieldCounter`
/// that, given a type `F: ff::Field`,
/// iterates over the multiples of `F::ONE`,
/// starting from some initial field element.
/// More precisely, for any `iter: FieldCounter`, it holds
/// ```ignore
/// let a = iter.next();
/// let b = iter.next();
/// assert_eq!(a + F::ONE, b);
/// ```
mod field_counter;
pub use field_counter::FieldCounter;

mod permutations_iter;
pub use permutations_iter::{inverse_permutation, PermutationsIter};

mod iter_apply_macro;

mod time_it_macro;

/// Simple auxiliary structs to be used in circuit tests.
/// Those are not optimized for use in actual scenarios,
/// but for ease of use in minimal test cases.
mod proving_utilities;
pub use proving_utilities::{ProverWrapper, VerifierWrapper};
