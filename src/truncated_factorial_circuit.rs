use halo2_proofs::{
    circuit::{Chip, Layouter, SimpleFloorPlanner, Value},
    plonk::{Column, Instance},
};

use crate::Number;

#[derive(Default)]
pub struct TruncatedFactorialCircuit<
    F: ff::Field,
    const N_FACTORS: usize,
    const MUL_BATCH_SIZE: usize,
    const N_COLUMNS: usize,
> {
    product_starting_from: Value<F>,
}

impl<F: ff::Field, const N_FACTORS: usize, const MUL_BATCH_SIZE: usize, const N_COLUMNS: usize>
    TruncatedFactorialCircuit<F, N_FACTORS, MUL_BATCH_SIZE, N_COLUMNS>
{
    pub fn new(first_factor: F) -> Self {
        Self {
            product_starting_from: Value::known(first_factor),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TFCircuitConfig<const N_COLUMNS: usize> {
    tf_config: crate::truncated_factorial_chip::TConfig<N_COLUMNS>,
    instance_column: Column<Instance>,
}

impl<F: ff::Field, const N_FACTORS: usize, const MUL_BATCH_SIZE: usize, const N_COLUMNS: usize>
    halo2_proofs::plonk::Circuit<F>
    for TruncatedFactorialCircuit<F, N_FACTORS, MUL_BATCH_SIZE, N_COLUMNS>
{
    type Config = TFCircuitConfig<N_COLUMNS>;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Default::default()
    }

    fn configure(meta: &mut halo2_proofs::plonk::ConstraintSystem<F>) -> Self::Config {
        let columns = [(); N_COLUMNS].map(|_| meta.advice_column());
        let instance_column = meta.instance_column();
        meta.enable_equality(instance_column);

        TFCircuitConfig {
            tf_config: crate::truncated_factorial_chip::TruncatedFactorialChip::<
                F,
                N_FACTORS,
                MUL_BATCH_SIZE,
                N_COLUMNS,
            >::configure(meta, columns),
            instance_column,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<F>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let factorial_chip = crate::truncated_factorial_chip::TruncatedFactorialChip::<
            F,
            N_FACTORS,
            MUL_BATCH_SIZE,
            N_COLUMNS,
        >::construct(config.tf_config);

        let input_item = layouter
            .namespace(|| "allocation of input item")
            .assign_region(
                || "allocation of input item",
                |mut region| {
                    region
                        .assign_advice(
                            || "input item",
                            factorial_chip.config().columns[0],
                            0,
                            || self.product_starting_from,
                        )
                        .map(Number)
                },
            )?;

        let output_item = factorial_chip.compute(
            layouter.namespace(|| "truncated factorial computation"),
            input_item,
        )?;

        layouter.namespace(|| "copy of output").constrain_instance(
            output_item.cell(),
            config.instance_column,
            0,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{
        dev::MockProver,
        pasta::Fp,
        plonk::{Circuit, ConstraintSystem},
    };

    /// Given that the circuit depends on a number of constant
    /// parameters that cannot be iterated over with variables,
    /// we use a macro to test that a range of different circuit
    /// configurations work as expected.
    ///
    /// The macro returns the result of proof verification.
    macro_rules! test_with_params {
        (<
            $N_FACTORS: literal,
            $MUL_BATCH_SIZE: literal,
            $N_COLUMNS: literal
        >(
            $initial_value: expr
        )[
            $expected_out: expr
        ]{
            $POW_OF_2_MAX_ROWS: expr
        }) => {{
            let circuit =
                TruncatedFactorialCircuit::<Fp, $N_FACTORS, $MUL_BATCH_SIZE, $N_COLUMNS>::new(
                    $initial_value,
                );

            let instance = vec![vec![$expected_out]];

            let prover = crate::time_it!(
                "Proof generation time: {:?}",
                MockProver::run($POW_OF_2_MAX_ROWS, &circuit, instance)
                    .expect("Proof generation goes wrong")
            );
            crate::time_it!("Proof verification time: {:?}", prover.verify())
        }};
    }

    #[test]
    /// Test the factorial circuit with the mock prover, which prints out errors and warnings.
    /// Given a number m, we prove that we know the product of the n numbers starting from it,
    /// with n in range (1..=20).
    /// If m is 1, as it is hard-coded here, we prove that we know the factorial of n.
    /// The circuit is also tested against invalid witness values.
    fn mock_factorial_1_to_20() {
        const POW_OF_2_MAX_ROWS: u32 = 6;

        /// Further information compression,
        /// built on top of the `test_with_params` macro
        macro_rules! batch_test_with_params_success {
            ($({$param: literal, $expected_out: expr})*) => {
                $(
                    test_with_params!(
                        <$param, 1, 1>
                        (Fp::from(1))
                        [$expected_out]
                        {POW_OF_2_MAX_ROWS}
                    ).expect("Proof verification goes wrong");
                )*
            };
        }

        /// Further information compression,
        /// built on top of the `test_with_params` macro
        macro_rules! batch_test_with_params_fail {
            ($({$param: literal, $expected_out: expr})*) => {
                $(
                    test_with_params!(
                        <$param, 1, 1>
                        (Fp::from(1))
                        [$expected_out]
                        {POW_OF_2_MAX_ROWS}
                    ).expect_err("Bogous proof was accepted");
                )*
            };
        }

        fn factorial(n: u64) -> Fp {
            Fp::from((1..=n).fold(1, |product, k| product * k))
        }

        batch_test_with_params_success!(
            {1, factorial(1)}
            {2, factorial(2)}
            {3, factorial(3)}
            {4, factorial(4)}
            {5, factorial(5)}
            {6, factorial(6)}
            {7, factorial(7)}
            {8, factorial(8)}
            {9, factorial(9)}
            {10, factorial(10)}
            {11, factorial(11)}
            {12, factorial(12)}
            {13, factorial(13)}
            {14, factorial(14)}
            {15, factorial(15)}
            {16, factorial(16)}
            {17, factorial(17)}
            {18, factorial(18)}
            {19, factorial(19)}
            {20, factorial(20)}
        );

        let three = Fp::from(3);
        // `3` is not a factorial, so testing the circuit
        // with any `N_FACTORS` such that the factorial
        // of `N_FACTORS` does not wrap around in the field,
        // should fail.
        batch_test_with_params_fail!(
            {1, three}
            {2, three}
            {3, three}
            {4, three}
            {5, three}
            {6, three}
            {7, three}
            {8, three}
            {9, three}
            {10, three}
            {11, three}
            {12, three}
            {13, three}
            {14, three}
            {15, three}
            {16, three}
            {17, three}
            {18, three}
            {19, three}
            {20, three}
        );
    }

    #[test]
    /// Test the factorial circuit with the mock prover, which prints out errors and warnings.
    /// We test the product of 1000 numbers, variating the starting number, the number of columns in the circuit,
    /// and the degree of the polynomial constraints imposed by the circuit.
    /// This was done for performance testing, to figure out halo2's internals with a trial-and-error approach.
    fn mock_factorial_1000() {
        /// This macro exists because [`iter_apply_macro`] requires
        /// a macro argument. It mostly behaves like a generic function,
        /// with `mul_batch_size` and `n_columns` as generic parameters
        /// and with `input_value` as argument.
        macro_rules! test1000 {
            (
                $mul_batch_size: literal,
                $n_columns: literal,
                $input_value: expr
            ) => {
                let base = Fp::from($input_value);
                let expected_out = (0..1000).fold(
                    Fp::from(1),
                    |product, increment| product * (base + Fp::from(increment))
                );

                // Since the minimum required number of rows for a circuit is
                // computed in a somewhat obscure way, we directly ask
                // the constraint system how many are needed.
                let pow_of_2_max_rows = {
                    let mut cs = ConstraintSystem::default();
                    TruncatedFactorialCircuit::<Fp, 1000, $mul_batch_size, $n_columns>::configure(&mut cs);
                    cs.minimum_rows().next_power_of_two().ilog2() + 1
                };

                println!(
                    "mul_batch_size = {} ; n_columns = {} ; input = {:?}",
                    $mul_batch_size,
                    $n_columns,
                    base
                );
                test_with_params!(
                    <1000, $mul_batch_size, $n_columns>
                    (base)
                    [expected_out]
                    {pow_of_2_max_rows}
                ).expect(stringify!(Proof verification goes wrong with MUL_BATCH_SIZE = $mul_batch_size ; N_COLUMNS = $n_columns));
            };
        }

        crate::iter_apply_macro!(
            test1000;
            [1, 2, 3, 4, 5, 6, 7, 8]
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
            [3_u64, 5_u64, 42_u64]
        );
    }

    #[test]
    /// Test the sudoku circuit with actual prover and verifier through the wrappers we implemented.
    /// This is very similar to a real use case.
    /// We prove that we know 1000! % m, where m is the maximum number Fp can represent.
    fn factorial() {
        const MAX_NR_ROWS_POW_2_EXPONENT: u32 = 4;
        const N_FACTORS: usize = 1000;

        use crate::utilities::{ProverWrapper, VerifierWrapper};

        let circuit_wiring = TruncatedFactorialCircuit::<Fp, N_FACTORS, 20, 10>::default();

        let mut prover = ProverWrapper::initialize_parameters_and_prover(
            MAX_NR_ROWS_POW_2_EXPONENT,
            circuit_wiring,
        )
        .expect("prover setup should not fail");
        let circuit = TruncatedFactorialCircuit::<Fp, N_FACTORS, 20, 10>::new(Fp::from(1));

        let instance = [(1..=N_FACTORS).fold(Fp::from(1), |acc, f| acc * Fp::from(f as u64))];
        let instance = [instance.as_slice()];

        prover.add_item(circuit, instance.as_slice());

        let transcript = crate::time_it! {
            "proof generation time: {:?}",
            prover.prove().expect("proof generation fails")
        };

        println!("Proof length in bytes: {}", transcript.len());

        let mut verifier = VerifierWrapper::from(prover);

        crate::time_it! {
            "proof verification time: {:?}",
            assert!(
                verifier.verify([instance.as_slice()], transcript.as_slice()),
                "proof verification falis"
            )
        };
    }
}
