#[derive(Debug, Clone, Copy, Default)]
pub struct DivModCounter<const MOD: usize, const RUNTIME_MOD: bool = false> {
    div: usize,
    remainder: usize,
    modulo: usize,
}

impl<const MOD: usize> DivModCounter<MOD, false> {
    pub fn new_const_mod(div: usize, remainder: usize) -> Self {
        assert!(MOD != 0, "divisor cannot be 0");
        Self {
            div,
            remainder: remainder % MOD,
            modulo: 0,
        }
    }
}

impl DivModCounter<0, true> {
    pub fn new_runtime_mod(div: usize, remainder: usize, modulo: usize) -> Self {
        assert!(modulo != 0, "divisor cannot be 0");
        Self {
            div,
            remainder: remainder % modulo,
            modulo,
        }
    }
}

impl<const MOD: usize, const RUNTIME_MOD: bool> Iterator for DivModCounter<MOD, RUNTIME_MOD> {
    type Item = (usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        self.remainder += 1;
        if self.remainder == if RUNTIME_MOD { self.modulo } else { MOD } {
            self.remainder = 0;
            self.div += 1;
        }
        Some((self.div, self.remainder))
    }
}
