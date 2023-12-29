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

The single circuits can be tested by matching the test name with `sudoku`, `permutation` or `factorial`.
The tests that match the `mock` pattern are written using the `MockProver` struct, while the others use the custom real-world provers.

### Running via Docker
To run the tests via Docker, the simplest way is to use the image published by CI:
```bash
docker run --rm --network host ghcr.io/libecciolabs/halo2-playground:main
```

You can always build the docker image yourself:
```bash
git clone https://github.com/LibeccioLabs/Halo2-Playground/
docker build -t halo2-playground ./Halo2-Playground/
docker run --rm halo2-playground
```

Since the Docker image will run `cargo test --release`, you can append any other flag, including a match for the tests you want to run.
For example, to run only the tests in the `sudoku` module, you can run:
```bash
docker run --rm halo2-playground sudoku
```
