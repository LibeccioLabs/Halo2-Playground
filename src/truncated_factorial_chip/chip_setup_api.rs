use halo2_proofs::circuit::Region;

use super::*;

impl<F: ff::Field, const N_FACTORS: usize, const MUL_BATCH_SIZE: usize, const N_COLUMNS: usize>
    TruncatedFactorialChip<F, N_FACTORS, MUL_BATCH_SIZE, N_COLUMNS>
{
    pub fn compute(
        &self,
        mut layouter: impl Layouter<F>,
        input_cell: Number<F>,
    ) -> Result<Number<F>, Error> {
        layouter.assign_region(
            || "assign factorial chip advice",
            |mut region| {
                let config = self.config();

                // We activate the gate
                config.s_fact.enable(&mut region, 0)?;

                // We build `assign_new_cell`, a closure that, given a value,
                // allocates it in the next available advice cell. The order of
                // the cells is consistent with the one in the gate implementation.
                let mut cell_counter = DivModCounter::new_runtime_mod(0, 0, N_COLUMNS)
                    .map(|(div, res)| (config.columns[res], div));
                let mut assign_new_cell = |region: &mut Region<'_, F>, value| {
                    let (column, offset) = cell_counter.next().expect("the iterator never ends");
                    region
                        .assign_advice(
                            || "truncated factorial advice cell",
                            column,
                            offset,
                            || value,
                        )
                        .map(Number)
                };

                // If the number of factors is 0, then the only constraint
                // imposed by the gate is that the first allocated cell
                // is set to `F::ONE`.
                if N_FACTORS == 0 {
                    return assign_new_cell(&mut region, Value::known(F::ONE));
                }
                // From now on, we know that `N_FACTORS > 0`. Keep it in mind!

                let input_value = input_cell.value().cloned();

                // Because of how the constraints in the gate are defined, the first
                // cell we have to allocate is a copy of the input value.
                // All the constraints are defined in terms of the value in this cell.
                //
                // Since we know the actual value inside it, i.e. `input_value`,
                // We don't have to bother using this variable.
                let _local_copy_of_input_cell = assign_new_cell(&mut region, input_value)?;

                // We make sure that the prover enforces that the
                // value we copied over is the same as the input.
                region.constrain_equal(input_cell.cell(), _local_copy_of_input_cell.cell())?;

                // An iterator that yields the sequence
                // of the terms to be multiplied in the factorial.
                let mut field_counter =
                    crate::utilities::FieldCounter::start_counting_from(F::ZERO)
                        .map(|f| input_value + Value::known(f));
                // A closure that integrates `batch_size`
                // new factors in the factorial product.
                let mut product_batch = |product_so_far, batch_size| {
                    (&mut field_counter)
                        .take(batch_size)
                        .fold(product_so_far, |prod, e| prod * e)
                };

                let mut product = Value::known(F::ONE);
                let mut output_cell = None;

                // As in the gate implementation, we add factors in groups of
                // `mul_batch_size`, until possible
                for _batch_nr in 0..N_FACTORS / MUL_BATCH_SIZE {
                    product = product_batch(product, MUL_BATCH_SIZE);
                    output_cell = Some(assign_new_cell(&mut region, product)?);
                }

                // Then, we apply a smaller batch for the remaining terms.
                if N_FACTORS % MUL_BATCH_SIZE != 0 {
                    product = product_batch(product, N_FACTORS % MUL_BATCH_SIZE);
                    output_cell = Some(assign_new_cell(&mut region, product)?);
                }

                Ok(
                    output_cell.expect(
                        "Since N_FACTORS > 0, by this point `output_cell` is not None, because `N_FACTORS / MUL_BATCH_SIZE > 0 || N_FACTORS % MUL_BATCH_SIZE != 0`"
                    )
                )
            },
        )
    }
}
