use super::permutation_chip::{PConfig, PermutationChip};

use super::Number;

use halo2_proofs::circuit::{Chip, Layouter};
use halo2_proofs::{
    circuit::Value,
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
        for idx in 0..N_OBJECTS {
            output_layouter.constrain_instance(
                permutation_cells[idx].0.cell(),
                config.instance,
                idx,
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A struct that iterates over all the permutations of a given length.
    struct KnuthL<const N_OBJECTS: usize>(Option<[usize; N_OBJECTS]>);

    impl<const N_OBJECTS: usize> Default for KnuthL<N_OBJECTS> {
        fn default() -> Self {
            if N_OBJECTS == 0 {
                return Self(None);
            }

            Self(Some(
                (0..N_OBJECTS).f_collect("the number of items is correct"),
            ))
        }
    }

    impl<const N_OBJECTS: usize> Iterator for KnuthL<N_OBJECTS> {
        type Item = [usize; N_OBJECTS];
        fn next(&mut self) -> Option<Self::Item> {
            if self.0 == None {
                return None;
            }
            // Copy the current state, as return value.
            let current = self.0;

            let array = self
                .0
                .as_mut()
                .expect("we checked at the beginning that self.0 != None");

            // Find last j such that self[j] <= self[j+1].
            // Nullify self.0 if it doesn't exist
            let j = (0..=N_OBJECTS - 2)
                .rev()
                .find(|&j| array[j] <= array[j + 1]);

            // The last permutation we yield is [N_OBJECTS - 1, N_OBJECTS - 2, ..., 1, 0]
            if j == None {
                self.0 = None;
                return current;
            }
            let j = j.unwrap();

            // Find last l such that self[j] <= self[l], then
            // exchange elements j and l, and then reverse self[j+1..]
            let l = (j + 1..=N_OBJECTS - 1).rev().find(|&l| array[j] <= array[l])
            .expect("since `j + 1` is in the range, and given the definition of `j`, we are sure that `find` will return `Some(...)`");
            array.swap(j, l);
            array[j + 1..].reverse();

            current
        }
    }

    /// Given a permutation, outputs its inverse.
    /// It is up to the caller to guarantee that the input
    /// to this function is an actual permutation.
    fn inverse_permutation<const N_OBJECTS: usize>(
        permutation: [usize; N_OBJECTS],
    ) -> [usize; N_OBJECTS] {
        let mut output = [0; N_OBJECTS];
        for (i, n) in permutation.into_iter().enumerate() {
            output[n] = i;
        }
        output
    }

    #[test]
    fn permutation_circuit_comprehensive_7_length_test() {
        use halo2_proofs::{dev::MockProver, pasta::Fp};

        let objects: [Value<Fp>; 7] = (0..7)
            .map(|n| Value::known(Fp::from(n as u64)))
            .f_collect("the number of items is correct");

        const POW_2_EXP_MAX_ROWS: u32 = 5;
        for permutation in KnuthL::<7>::default() {
            let circuit = PermutationCircuit::<Fp, 7>::new_unchecked(objects.clone(), permutation);

            // The permutation output is equal to the permutation that is
            // inverse to `permutation`
            let permutation_output =
                Vec::from(inverse_permutation(permutation).map(|x| Fp::from(x as u64)));

            let prover = MockProver::run(POW_2_EXP_MAX_ROWS, &circuit, vec![permutation_output])
                .expect("Proof generation goes wrong");

            assert_eq!(prover.verify(), Ok(()), "Proof verification goes wrong");
        }
    }

    #[test]
    fn permutation_circuit_test_with_actual_prover() {
        use halo2_proofs::pasta::{EqAffine, Fp};

        const N_OBJECTS: usize = 5;
        const N_OBJECTS_FACTORIAL: usize = 120;
        type TestCircuit = PermutationCircuit<Fp, N_OBJECTS>;

        /// This constant controls the maximum number of rows available in each circuit.
        /// If K is too low, the proof generation fails.
        ///
        /// Choosing a smaller K improves proving times, by a lot.
        ///
        /// Currently, we do not know if choosing a bigger value has advantages.
        const K: u32 = 4;

        let objects: [Value<Fp>; 5] = (0..N_OBJECTS)
            .map(|n| Value::known(Fp::from(n as u64)))
            .f_collect("the number of items is correct");

        let public_parameters = halo2_proofs::poly::commitment::Params::<EqAffine>::new(K);

        let circuit = TestCircuit::default();

        let vk = halo2_proofs::plonk::keygen_vk(&public_parameters, &circuit)
            .expect("verifier key generation should not fail");
        let pk = halo2_proofs::plonk::keygen_pk(&public_parameters, vk.clone(), &circuit)
            .expect("proving key generation should not fail");

        // Apparently, we can generate batches of proofs with Halo2. Neat!
        // All we need to do is put our circuits and instances in a slice,
        // before feeding them to the prover.

        let circuits: [TestCircuit; N_OBJECTS_FACTORIAL] = KnuthL::<5>::default()
            .map(|permutation| TestCircuit::new_unchecked(objects.clone(), permutation))
            .f_collect("the number of items is correct");

        let instances: [[Fp; N_OBJECTS]; N_OBJECTS_FACTORIAL] = KnuthL::<5>::default()
            .map(|permutation| inverse_permutation(permutation).map(|x| Fp::from(x as u64)))
            .f_collect("the number of items is correct");

        // We need to do some formatting with the instances before giving them to the prover.
        // What we have is [[Fp; N_OBJECTS]; _]. What the prover wants is &[&[&[Fp]]]

        //
        let instances: [[&[Fp]; 1]; N_OBJECTS_FACTORIAL] = instances
            .iter()
            .map(|x| [x.as_slice()])
            .f_collect("the number of items is correct");
        let instances: [&[&[Fp]]; N_OBJECTS_FACTORIAL] = instances
            .iter()
            .map(|x| x.as_slice())
            .f_collect("the number of items is correct");

        // Why Blake2bWrite ? Because it is the only type in halo2_proofs
        // that implements the `halo2_proosf::transcript::TranscriptWrite`
        // trait, which is needed for the transcript argument to
        // `halo2_proofs::plonk::create_proof`
        //
        // Why use EqAffine as generic parameter? Idk. Would be nice to know.
        let mut transcript =
            halo2_proofs::transcript::Blake2bWrite::<_, halo2_proofs::pasta::EqAffine, _>::init(
                vec![],
            );

        crate::time_it! {
                "The proving time of 120 5-items permutations with an actual prover is {:?}",
            halo2_proofs::plonk::create_proof(
                &public_parameters,
                &pk,
                &circuits,
                &instances,
                // OsRng is assumed to be cryptographically safe. Also kinda slow.
                rand::rngs::OsRng,
                &mut transcript,
            )
            .expect("proof generation should not fail");
        }

        let transcript = transcript.finalize();
        println!(
            "The aggregated proof's length is {} bytes",
            transcript.len()
        );
        let mut transcript =
            halo2_proofs::transcript::Blake2bRead::<_, halo2_proofs::pasta::EqAffine, _>::init(
                transcript.as_slice(),
            );

        let strategy = halo2_proofs::plonk::SingleVerifier::new(&public_parameters);

        crate::time_it! {
            "And the verification time with an actual verifier is {:?}",
            halo2_proofs::plonk::verify_proof(
                &public_parameters,
                &vk,
                strategy,
                &instances,
                &mut transcript,
            )
            .expect("proof verification should not fail");
        }
    }
}
