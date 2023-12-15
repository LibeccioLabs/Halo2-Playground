use super::*;

impl<F: ff::Field, const N_FACTORS: usize, const MUL_BATCH_SIZE: usize, const N_COLUMNS: usize>
    TruncatedFactorialChip<F, N_FACTORS, MUL_BATCH_SIZE, N_COLUMNS>
{
    pub fn construct(config: <Self as halo2_proofs::circuit::Chip<F>>::Config) -> Self {
        Self {
            config,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        columns: [Column<Advice>; N_COLUMNS],
    ) -> <Self as halo2_proofs::circuit::Chip<F>>::Config {
        assert!(
            N_COLUMNS > 0,
            "At least one column to allocate multiplication constraints is needed."
        );

        assert!(
            MUL_BATCH_SIZE > 0,
            "Multiplications have to be batched in groups of at least one at a time."
        );

        let s_fact = meta.selector();

        for col in columns.iter() {
            meta.enable_equality(*col);
        }

        meta.create_gate("partial factorial gate", |meta| {
            let s_fact = meta.query_selector(s_fact);

            let mut next_cell_iter = DivModCounter::new_runtime_mod(0, 0, N_COLUMNS)
                .into_iter()
                .map(|(div, rem)| meta.query_advice(columns[rem], Rotation(div as i32)));
            let mut next_cell = || next_cell_iter.next().expect("the iterator never ends");

            let first_cell = next_cell();

            if N_FACTORS == 0 {
                return vec![s_fact * (first_cell - Expression::Constant(F::ONE))];
            }

            let mut field_counter = crate::utilities::FieldCounter::start_counting_from(F::ZERO);

            let mut constraints = vec![];

            let mut last_cell = Expression::Constant(F::ONE);

            let mut batch_multiply = |batch_size| {
                // product is a chunk of the factors that appear in the factorial.
                // Which factors they are is kept track in the state of `field_counter`.
                let product = (&mut field_counter).take(batch_size).fold(
                    Expression::Constant(F::ONE),
                    |product, increment| {
                        product * (first_cell.clone() + Expression::Constant(increment))
                    },
                );

                let next_cell = next_cell();

                // We force the next cell to be equal to
                // the updated value of aggregated product
                constraints
                    .push(s_fact.clone() * (next_cell.clone() - last_cell.clone() * product));
                last_cell = next_cell;
            };

            for _batch_nr in 0..N_FACTORS / MUL_BATCH_SIZE {
                batch_multiply(MUL_BATCH_SIZE);
            }

            if N_FACTORS % MUL_BATCH_SIZE != 0 {
                batch_multiply(N_FACTORS % MUL_BATCH_SIZE);
            }

            constraints
        });

        TConfig { columns, s_fact }
    }
}
