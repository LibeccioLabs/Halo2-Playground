#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct FieldCounter<F: ff::Field> {
    state: F,
}

impl<F: ff::Field> Iterator for FieldCounter<F> {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        let r = self.state;
        self.state += F::ONE;
        Some(r)
    }
}

impl<F: ff::Field> FieldCounter<F> {
    pub fn current(&self) -> F {
        self.state
    }

    pub fn start_counting_from(initial_state: F) -> Self {
        Self {
            state: initial_state,
        }
    }
}
