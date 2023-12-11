# Halo2 Playground

This crate contains the implementation of a circuit that builds a
Zero Knowledge Proof of the fact that the prover knows a solution
to a given sudoku problem.

We wrote this crate to get a first-hand feeling of the user friendlyness
of the [Halo2](https://github.com/zcash/halo2) proving system.

## Running the tests
To run the tests, run the command `cargo test --release`.  
The command `cargo test` works too, but in that case you may want
to give your computer a couple of minutes to compute the test results.  
The flag `-- --nocapture` can be used to print the execution times for proof generation and verification.