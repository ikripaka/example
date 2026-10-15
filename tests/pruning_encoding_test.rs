use simplex::transaction::{
    FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature,
};
use simplex_example_2::artifacts::minimal_pruning::MinimalPruningProgram;
use simplex_example_2::artifacts::minimal_pruning::derived_minimal_pruning::MinimalPruningArguments;
use simplicityhl::simplicity::dag::InternalSharing;
use simplicityhl::{
    Arguments, CompiledProgram, ResolvedType, SatisfiedProgram, TemplateProgramWitness,
    UnstableFeatures, Value, WitnessNameToValueMap, WitnessValues,
    ast::ElementsJetHinter,
    dummy_env,
    num::U256,
    resolution::DependencyMapBuilder,
    simplicity::{BitIter, RedeemNode, dag::DagLike, jet::Elements},
    source::{CanonPath, CanonSourceFile},
    types::TypeConstructible,
    value::ValueConstructible,
};
use std::{collections::HashMap, path::Path, sync::Arc};

fn get_witness(check_none: bool) -> WitnessValues {
    WitnessValues::from_map(HashMap::from([
        (
            TemplateProgramWitness::witness_from_str("CHECK_NONE"),
            Value::from(check_none),
        ),
        (
            TemplateProgramWitness::witness_from_str("FIRST"),
            if check_none {
                Value::none(ResolvedType::u256())
            } else {
                Value::some(Value::u256(U256::from(0u8)))
            },
        ),
        (
            TemplateProgramWitness::witness_from_str("SECOND"),
            Value::some(Value::u256(U256::from(0u8))),
        ),
    ]))
}

fn satisfied_program(witness: WitnessValues) -> anyhow::Result<SatisfiedProgram> {
    const DEBUG_SYMBOLS: bool = false;

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("simf");
    let path = root.join("minimal_pruning.simf");
    let source = CanonSourceFile::new(
        CanonPath::canonicalize(&path).map_err(anyhow::Error::msg)?,
        Arc::from(std::fs::read_to_string(path)?),
    );

    let dependencies = DependencyMapBuilder::new()
        .build(CanonPath::canonicalize(&root).map_err(anyhow::Error::msg)?)?;

    let compiled = CompiledProgram::new_with_dep(
        source,
        &dependencies,
        &UnstableFeatures::all(),
        Arguments::default(),
        DEBUG_SYMBOLS,
        Box::new(ElementsJetHinter::new()),
    )
    .map_err(anyhow::Error::msg)?;

    compiled.satisfy(witness).map_err(anyhow::Error::msg)
}

#[test]
fn pruning_decoded_input_discards_inactive_witness() -> anyhow::Result<()> {
    let witness = get_witness(false);
    let satisfied = satisfied_program(witness)?;
    let (program, witness) = satisfied.redeem().to_vec_with_witness();

    let shared_input = RedeemNode::decode::<_, _, Elements>(
        BitIter::from(program.as_slice()),
        BitIter::from(witness.as_slice()),
    )?;
    let pruned = shared_input.prune(&dummy_env::dummy())?;

    let (program, witness) = pruned.to_vec_with_witness();
    let prog_iter = BitIter::from(program.as_slice());
    let wit_iter = BitIter::from(witness.as_slice());

    RedeemNode::decode::<_, _, Elements>(prog_iter, wit_iter)?;

    assert_eq!(pruned.cmr(), shared_input.cmr());
    Ok(())
}

mod fails {
    use super::*;

    // like here https://github.com/BlockstreamResearch/rust-simplicity/blob/546bafee43636cf55622f8f1f75f8f12cd034459/src/node/redeem.rs#L1046
    #[test]
    fn pruning_is_idempotent_and_preserves_maximal_sharing() -> anyhow::Result<()> {
        println!("=== [fails] pruning_is_idempotent ===");
        let witness = get_witness(true);

        println!("Arguments: empty; Witness: `{witness:?}`");
        let satisfied = satisfied_program(witness)?;

        let env = dummy_env::dummy();

        let once = satisfied.redeem().prune(&env)?;
        let (once_program, once_witness) = once.to_vec_with_witness();
        println!(
            "once: {}, witness: {}, program: {}",
            once.cmr(),
            hex::encode(&once_witness),
            hex::encode(&once_program)
        );

        let twice = once.prune(&env)?;
        let (twice_program, twice_witness) = twice.to_vec_with_witness();
        println!(
            "twice: {}, witness: {}, program: {}",
            twice.cmr(),
            hex::encode(&twice_witness),
            hex::encode(&twice_program)
        );

        assert_eq!(once.to_vec_with_witness(), twice.to_vec_with_witness());
        assert!(once.as_ref().is_shared_as::<InternalSharing>());
        assert!(twice.as_ref().is_shared_as::<InternalSharing>());
        // assert!(once.as_ref().is_shared_as::<MaxSharing<Redeem>>());
        // assert!(twice.as_ref().is_shared_as::<MaxSharing<Redeem>>());
        Ok(())
    }

    #[simplex::test]
    fn min_pruning_simplex(context: simplex::TestContext) -> anyhow::Result<()> {
        let arguments = MinimalPruningArguments {};
        let program = MinimalPruningProgram::new(arguments.clone());

        let witness = get_witness(false);

        let signer = context.get_default_signer();
        let script = program.get_script_pubkey(context.get_network());

        println!("=== [fails] min_pruning_simplex === ");
        println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

        signer.send(script.clone(), 50)?.wait()?;
        let utxo = context
            .get_default_provider()
            .fetch_scripthash_utxos(&script)?
            .into_iter()
            .next()
            .unwrap();

        let mut transaction = FinalTransaction::new();
        transaction.add_program_input(
            PartialInput::new(utxo),
            ProgramInput::new(Box::new(program.as_ref().clone()), witness),
            RequiredSignature::None,
        );
        transaction.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            49,
            context.get_network().policy_asset(),
        ));

        signer.broadcast(&transaction)?.wait()?;
        Ok(())
    }

    // like here https://github.com/BlockstreamResearch/rust-simplicity/blob/546bafee43636cf55622f8f1f75f8f12cd034459/fuzz/fuzz_targets/c_rust_merkle.rs#L39
    #[test]
    fn pruning_keep_equal_cmr() -> anyhow::Result<()> {
        println!("=== [fails] pruning_keep_equal_cmr === ");

        let witness = get_witness(true);
        let satisfied = satisfied_program(witness)?;

        let env = dummy_env::dummy();
        let pruned = satisfied.redeem().prune(&env)?;

        let (program, witness) = pruned.to_vec_with_witness();

        println!("pruned cmr: {}", pruned.cmr());
        println!(
            "witness: {}, program: {}",
            hex::encode(&witness),
            hex::encode(&program)
        );

        let prog_iter = BitIter::from(program.as_slice());
        let wit_iter = BitIter::from(witness.as_slice());

        let decoded = RedeemNode::decode::<_, _, Elements>(prog_iter, wit_iter)?;

        simplicityhl::simplicity::BitMachine::for_program(&decoded)?.exec(&decoded, &env)?;
        assert_eq!(decoded.cmr(), pruned.cmr());
        Ok(())
    }
}

mod works {
    use super::*;

    // like here https://github.com/BlockstreamResearch/rust-simplicity/blob/546bafee43636cf55622f8f1f75f8f12cd034459/src/node/redeem.rs#L1046
    #[test]
    fn pruning_is_idempotent() -> anyhow::Result<()> {
        println!("=== [works] pruning_is_idempotent ===");
        let witness = get_witness(false);

        println!("Arguments: empty; Witness: `{witness:?}`");
        let satisfied = satisfied_program(witness)?;

        let env = dummy_env::dummy();

        let once = satisfied.redeem().prune(&env)?;
        let (once_program, once_witness) = once.to_vec_with_witness();
        println!(
            "once: {}, witness: {}, program: {}",
            once.cmr(),
            hex::encode(&once_witness),
            hex::encode(&once_program)
        );

        let twice = once.prune(&env)?;
        let (twice_program, twice_witness) = twice.to_vec_with_witness();
        println!(
            "twice: {}, witness: {}, program: {}",
            twice.cmr(),
            hex::encode(&twice_witness),
            hex::encode(&twice_program)
        );

        assert_eq!(once.to_vec_with_witness(), twice.to_vec_with_witness());
        assert!(once.as_ref().is_shared_as::<InternalSharing>());
        assert!(twice.as_ref().is_shared_as::<InternalSharing>());
        // assert!(once.as_ref().is_shared_as::<MaxSharing<Redeem>>());
        // assert!(twice.as_ref().is_shared_as::<MaxSharing<Redeem>>());
        Ok(())
    }

    #[simplex::test]
    fn min_pruning_simplex(context: simplex::TestContext) -> anyhow::Result<()> {
        let arguments = MinimalPruningArguments {};
        let program = MinimalPruningProgram::new(arguments.clone());

        let witness = get_witness(false);

        let signer = context.get_default_signer();
        let script = program.get_script_pubkey(context.get_network());

        println!("=== [works] min_pruning_simplex === ");
        println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

        signer.send(script.clone(), 50)?.wait()?;
        let utxo = context
            .get_default_provider()
            .fetch_scripthash_utxos(&script)?
            .into_iter()
            .next()
            .unwrap();

        let mut transaction = FinalTransaction::new();
        transaction.add_program_input(
            PartialInput::new(utxo),
            ProgramInput::new(Box::new(program.as_ref().clone()), witness),
            RequiredSignature::None,
        );
        transaction.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            49,
            context.get_network().policy_asset(),
        ));

        signer.broadcast(&transaction)?.wait()?;
        Ok(())
    }

    // like here https://github.com/BlockstreamResearch/rust-simplicity/blob/546bafee43636cf55622f8f1f75f8f12cd034459/fuzz/fuzz_targets/c_rust_merkle.rs#L39
    #[test]
    fn pruning_keep_equal_cmr() -> anyhow::Result<()> {
        println!("=== [works] pruning_keep_equal_cmr === ");

        let witness = get_witness(false);
        let satisfied = satisfied_program(witness)?;

        let env = dummy_env::dummy();
        let pruned = satisfied.redeem().prune(&env)?;

        let (program, witness) = pruned.to_vec_with_witness();

        println!("pruned cmr: {}", pruned.cmr());
        println!(
            "witness: {}, program: {}",
            hex::encode(&witness),
            hex::encode(&program)
        );

        let prog_iter = BitIter::from(program.as_slice());
        let wit_iter = BitIter::from(witness.as_slice());

        let decoded = RedeemNode::decode::<_, _, Elements>(prog_iter, wit_iter)?;

        simplicityhl::simplicity::BitMachine::for_program(&decoded)?.exec(&decoded, &env)?;
        assert_eq!(decoded.cmr(), pruned.cmr());
        Ok(())
    }
}
