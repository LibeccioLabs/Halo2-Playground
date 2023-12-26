use std::marker::PhantomData;

use halo2_proofs::{
    pasta::{EqAffine, Fp},
    plonk::{Circuit, Error, ProvingKey, SingleVerifier, VerifyingKey},
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite},
};

pub struct ProverWrapper<'i, C: Circuit<Fp>> {
    public_parameters: Params<EqAffine>,
    /// The prover does not use this value, but it is necessary to provide
    /// a `VerifierWrapper: From<ProverWrapper>` implementation
    verifying_key: VerifyingKey<EqAffine>,
    proving_key: ProvingKey<EqAffine>,
    circuits: Vec<C>,
    instances: Vec<&'i [&'i [Fp]]>,
}

impl<'i, C: Circuit<Fp>> ProverWrapper<'i, C> {
    pub fn initialize_parameters_and_prover(
        max_nr_rows_pow_2_exponent: u32,
        circuit_wiring: C,
    ) -> Result<Self, Error> {
        let public_parameters = Params::new(max_nr_rows_pow_2_exponent);
        Self::initialize_prover(public_parameters, circuit_wiring)
    }

    pub fn initialize_prover(
        public_parameters: Params<EqAffine>,
        circuit_wiring: C,
    ) -> Result<Self, Error> {
        let verifying_key = halo2_proofs::plonk::keygen_vk(&public_parameters, &circuit_wiring)?;
        let proving_key = halo2_proofs::plonk::keygen_pk(
            &public_parameters,
            verifying_key.clone(),
            &circuit_wiring,
        )?;
        Ok(Self {
            public_parameters,
            verifying_key,
            proving_key,
            circuits: vec![],
            instances: vec![],
        })
    }

    pub fn add_item(&mut self, circuit: C, instance: &'i [&'i [Fp]]) {
        self.circuits.push(circuit);
        self.instances.push(instance);
    }

    pub fn clear(&mut self) {
        self.circuits.clear();
        self.instances.clear();
    }

    pub fn prove(&self) -> Result<Vec<u8>, Error> {
        let mut transcript = Blake2bWrite::init(vec![]);

        halo2_proofs::plonk::create_proof(
            &self.public_parameters,
            &self.proving_key,
            self.circuits.as_slice(),
            self.instances.as_slice(),
            rand::rngs::OsRng,
            &mut transcript,
        )?;

        Ok(transcript.finalize())
    }

    pub fn public_parameters(&self) -> &Params<EqAffine> {
        &self.public_parameters
    }

    pub fn proving_key(&self) -> &ProvingKey<EqAffine> {
        &self.proving_key
    }

    pub fn inner_parts(self) -> (Params<EqAffine>, ProvingKey<EqAffine>) {
        (self.public_parameters, self.proving_key)
    }

    pub fn from_inner_parts(
        public_parameters: Params<EqAffine>,
        verifying_key: VerifyingKey<EqAffine>,
        proving_key: ProvingKey<EqAffine>,
    ) -> Self {
        Self {
            public_parameters,
            verifying_key,
            proving_key,
            circuits: vec![],
            instances: vec![],
        }
    }
}

pub struct VerifierWrapper<C: Circuit<Fp>> {
    public_parameters: Params<EqAffine>,
    verifying_key: VerifyingKey<EqAffine>,
    _phantom: PhantomData<C>,
}

impl<C: Circuit<Fp>> VerifierWrapper<C> {
    pub fn initialize_verifier(
        public_parameters: Params<EqAffine>,
        circuit_wiring: C,
    ) -> Result<Self, Error> {
        let verifying_key = halo2_proofs::plonk::keygen_vk(&public_parameters, &circuit_wiring)?;
        Ok(Self {
            public_parameters,
            verifying_key,
            _phantom: PhantomData,
        })
    }

    pub fn verify<'i, I: IntoIterator<Item = &'i [&'i [Fp]]>>(
        &mut self,
        instances: I,
        transcript: &[u8],
    ) -> bool {
        let instances = Vec::from_iter(instances.into_iter());

        let mut transcript = Blake2bRead::init(transcript);
        let strategy = SingleVerifier::new(&self.public_parameters);
        halo2_proofs::plonk::verify_proof(
            &self.public_parameters,
            &self.verifying_key,
            strategy,
            instances.as_slice(),
            &mut transcript,
        )
        .is_ok()
    }

    pub fn from_inner_parts(
        public_parameters: Params<EqAffine>,
        verifying_key: VerifyingKey<EqAffine>,
    ) -> Self {
        Self {
            public_parameters,
            verifying_key,
            _phantom: PhantomData,
        }
    }
}

impl<'i, C: Circuit<Fp>> From<ProverWrapper<'i, C>> for VerifierWrapper<C> {
    fn from(value: ProverWrapper<'i, C>) -> Self {
        Self::from_inner_parts(value.public_parameters, value.verifying_key)
    }
}
