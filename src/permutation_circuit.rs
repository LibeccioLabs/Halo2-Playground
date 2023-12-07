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
    #[allow(non_snake_case)]
    fn Knuth_L(a: &mut [usize]) -> Option<()> {
        if a.len() <= 1 {
            return None;
        }
        // Find last j such that self[j] <= self[j+1]. Terminate if no such j exists.
        let j = (0..=a.len() - 2).rev().find(|&j| a[j] <= a[j + 1])?;

        // Find last l such that self[j] <= self[l], then exchange elements j and l.
        a.swap(j, (j + 1..=a.len() - 1).rev().find(|&l| a[j] <= a[l])?);
        a[j + 1..].reverse();
        Some(())
    }

    #[test]
    fn permutation_circuit_comprehensive_7_length_test() {
        use halo2_proofs::{dev::MockProver, pasta::Fp};

        let mut permutation = [0; 7];
        for i in 0..7 {
            permutation[i] = i;
        }

        let objects = permutation
            .clone()
            .map(|n| Value::known(Fp::from(n as u64)));

        const POW_2_EXP_MAX_ROWS: u32 = 5;
        loop {
            let circuit = PermutationCircuit::<Fp, 7>::new_unchecked(objects.clone(), permutation);

            // The permutation output is equal to the permutation that is
            // inverse to `permutation`
            let mut permutation_output = [0; 7];
            for (i, p) in permutation.into_iter().enumerate() {
                permutation_output[p] = i;
            }
            let permutation_output = Vec::from(permutation_output.map(|x| Fp::from(x as u64)));

            let prover = MockProver::run(POW_2_EXP_MAX_ROWS, &circuit, vec![permutation_output])
                .expect("Proof generation goes wrong");

            assert_eq!(prover.verify(), Ok(()), "Proof verification goes wrong");

            if Knuth_L(&mut permutation).is_none() {
                break;
            }
        }
    }
}
