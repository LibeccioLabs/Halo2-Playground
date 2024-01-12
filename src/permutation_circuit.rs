use crate::{
    permutation_chip::{PConfig, PermutationChip},
    Number,
};

use halo2_proofs::{
    circuit::{Chip, Layouter, Value},
    plonk::{Column, ConstraintSystem, Error, Instance},
};
use try_collect::{ForceCollect, TryCollect, TryFromIterator};

/// A circuit that proves that the input and output values are a permutation of one another.
pub struct PermutationCircuit<F: ff::Field, const N_OBJECTS: usize> {
    input_items: [Value<F>; N_OBJECTS],
    permutation: [usize; N_OBJECTS],
}

impl<F: ff::Field, const N_OBJECTS: usize> PermutationCircuit<F, N_OBJECTS> {
    pub fn new_unchecked(
        input_items: [Value<F>; N_OBJECTS],
        permutation: [usize; N_OBJECTS],
    ) -> Self {
        Self {
            input_items,
            permutation,
        }
    }

    pub fn try_new<I: IntoIterator, J: IntoIterator>(
        input_items: I,
        permutation: J,
    ) -> Result<
        Self,
        (
            Option<<[Value<F>; N_OBJECTS] as TryFromIterator<I::Item>>::Error>,
            Option<<[usize; N_OBJECTS] as TryFromIterator<J::Item>>::Error>,
            bool,
        ),
    >
    where
        [Value<F>; N_OBJECTS]: TryFromIterator<I::Item>,
        [usize; N_OBJECTS]: TryFromIterator<J::Item>,
    {
        Ok(Self {
            input_items: input_items
                .try_collect()
                .map_err(|err| (Some(err), None, false))?,
            permutation: Self::permutation_cosistency_check(
                permutation
                    .try_collect()
                    .map_err(|err| (None, Some(err), false))?,
            )
            .map_err(|_| (None, None, true))?,
        })
    }

    fn permutation_cosistency_check(p: [usize; N_OBJECTS]) -> Result<[usize; N_OBJECTS], ()> {
        let mut q: [u8; N_OBJECTS] = [0; N_OBJECTS];
        for i in p {
            q[i] += 1;
        }
        if !q.into_iter().all(|count| count == 1) {
            Err(())
        } else {
            Ok(p)
        }
    }
}

impl<F: ff::Field, const N_OBJECTS: usize> Default for PermutationCircuit<F, N_OBJECTS> {
    fn default() -> Self {
        Self {
            input_items: [Value::unknown(); N_OBJECTS],
            permutation: [0; N_OBJECTS],
        }
    }
}

#[derive(Debug, Clone)]
pub struct PCircuitConfig<const N_OBJECTS: usize> {
    pconfig: PConfig<N_OBJECTS>,
    instance: Column<Instance>,
}

impl<F: ff::Field, const N_OBJECTS: usize> halo2_proofs::plonk::Circuit<F>
    for PermutationCircuit<F, N_OBJECTS>
{
    type Config = PCircuitConfig<N_OBJECTS>;
    type FloorPlanner = halo2_proofs::circuit::SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let instance = meta.instance_column();
        meta.enable_equality(instance);

        let item_columns = (0..N_OBJECTS)
            .map(|_| meta.advice_column())
            .f_collect("the number of items is correct");
        let swap_selector_columns = (0..N_OBJECTS / 2).map(|_| meta.advice_column()).collect();

        PCircuitConfig {
            pconfig: PermutationChip::configure(meta, item_columns, swap_selector_columns),
            instance,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        let permutation_chip = PermutationChip::<N_OBJECTS, F>::construct(config.pconfig);

        // We assign the input values to the first row of the `item_columns` advice columns
        let input_cells = layouter.namespace(|| "input values").assign_region(
            || "input values",
            |mut region| {
                let item_columns = permutation_chip.config().get_item_columns();

                (0..N_OBJECTS)
                    .map(|idx| {
                        region
                            .assign_advice(
                                || format!("{idx}-th input value"),
                                item_columns[idx],
                                0,
                                || self.input_items[idx],
                            )
                            .map(Number)
                    })
                    .try_collect::<[Number<F>; N_OBJECTS]>()
                    .map_err(|err| match err {
                        try_collect::ArrayAndTupleError::TryFromError(err) => err,
                        _ => unreachable!("we are sure that the item count is correct"),
                    })
            },
        )?;

        // Now we call the chip's API to assign all the values required to
        // obtain the output permutation, and to prove that it is indeed
        // the required permutation
        let permutation_cells = permutation_chip.apply_permutation(
            layouter.namespace(|| "permutation chip assignment"),
            input_cells,
            self.permutation,
        )?;

        let mut output_layouter = layouter.namespace(|| "public output assignment");
        for (idx, cell) in permutation_cells.iter().enumerate().take(N_OBJECTS) {
            output_layouter.constrain_instance(cell.0.cell(), config.instance, idx)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utilities::{inverse_permutation, PermutationsIter};

    #[test]
    /// Test the permutation circuit with the mock prover, which prints out errors and warnings.
    /// We prove that every possible permutation of 7 items is correctly proved.
    fn mock_permutation() {
        use halo2_proofs::{dev::MockProver, pasta::Fp};

        let objects: [Value<Fp>; 7] = core::array::from_fn(|n| Value::known(Fp::from(n as u64)));

        const POW_2_EXP_MAX_ROWS: u32 = 5;
        for permutation in PermutationsIter::<7> {
            let circuit = PermutationCircuit::<Fp, 7>::new_unchecked(objects.clone(), permutation);

            // The permutation output is equal to the permutation that is inverse to `permutation`
            let permutation_output =
                Vec::from(inverse_permutation(permutation).map(|x| Fp::from(x as u64)));

            let prover = MockProver::run(POW_2_EXP_MAX_ROWS, &circuit, vec![permutation_output])
                .expect("Proof generation goes wrong");

            assert_eq!(prover.verify(), Ok(()), "Proof verification goes wrong");
        }
    }

    #[test]
    /// Test the permutation circuit with actual prover and verifier through the wrappers we implemented.
    /// This is very similar to a real use case.
    /// We prove that every possible permutation of 5 items is correctly proved.
    fn permutation() {
        use halo2_proofs::pasta::Fp;

        use crate::utilities::{ProverWrapper, VerifierWrapper};

        const N_OBJECTS: usize = 5;
        const FACTORIAL: usize = 120;

        /// This constant controls the maximum number of rows available in each circuit.
        /// If K is too low, the proof generation fails.
        ///
        /// Choosing a smaller K improves proving times, by a lot.
        ///
        /// Currently, we do not know if choosing a bigger value has advantages.
        const K: u32 = 4;

        let objects: [Value<Fp>; N_OBJECTS] =
            core::array::from_fn(|n| Value::known(Fp::from(n as u64)));

        let circuit_wiring = PermutationCircuit::<Fp, N_OBJECTS>::default();
        let mut prover = ProverWrapper::initialize_parameters_and_prover(K, circuit_wiring)
            .expect("prover setup should not fail");

        // For every circuit instance, we need to provide the set of public inputs of that instance.
        // We have `FACTORIAL` instances, with one column per instance.
        let instances: [[Fp; N_OBJECTS]; FACTORIAL] = PermutationsIter::<N_OBJECTS>
            .into_iter()
            .map(|permutation| inverse_permutation(permutation).map(|x| Fp::from(x as u64)))
            .f_collect("the number of items is correct");
        let instance_slices: [[&[Fp]; 1]; FACTORIAL] =
            core::array::from_fn(|i| [instances[i].as_slice()]);

        for (instance, permutation) in instance_slices.iter().zip(PermutationsIter::<5>) {
            let circuit =
                PermutationCircuit::<Fp, N_OBJECTS>::new_unchecked(objects.clone(), permutation);

            prover.add_item(circuit, instance.as_slice());
        }

        let transcript = crate::time_it! {
            "The proving time of 120 5-items permutations with an actual prover is {:?}",
            prover.prove().expect("proof generation should not fail")
        };

        println!(
            "The aggregated proof's length is {} bytes",
            transcript.len()
        );

        let mut verifier = VerifierWrapper::from(prover);

        crate::time_it! {
            "And the verification time with an actual verifier is {:?}",
            assert!(verifier.verify(instance_slices.iter().map(|a| a.as_slice()), transcript.as_slice()))
        }
    }
}
