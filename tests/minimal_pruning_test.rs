use std::{collections::HashMap, path::Path, sync::Arc};

use simplicityhl::{
    Arguments, CompiledProgram, ResolvedType, TemplateProgramWitness, UnstableFeatures, Value,
    WitnessNameToValueMap, WitnessValues,
    ast::ElementsJetHinter,
    dummy_env,
    num::U256,
    resolution::DependencyMapBuilder,
    simplicity::{BitIter, BitMachine, RedeemNode, jet::Elements},
    source::{CanonPath, CanonSourceFile},
    types::TypeConstructible,
    value::ValueConstructible,
};

// Fails
#[test]
fn minimal_pruning_encoding_roundtrip() -> anyhow::Result<()> {
    let arguments = Arguments::from_map(HashMap::new());
    let witness = WitnessValues::from_map(HashMap::from([
        (
            TemplateProgramWitness::witness_from_str("CHECK_NONE"),
            Value::from(true),
        ),
        (
            TemplateProgramWitness::witness_from_str("FIRST"),
            Value::none(ResolvedType::u256()),
        ),
        (
            TemplateProgramWitness::witness_from_str("SECOND"),
            Value::some(Value::u256(U256::from(0u8))),
        ),
    ]));

    println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("simf");
    let path = root.join("minimal_pruning.simf");
    let source = CanonSourceFile::new(
        CanonPath::canonicalize(&path).map_err(anyhow::Error::msg)?,
        Arc::from(std::fs::read_to_string(&path)?),
    );
    let dependencies = DependencyMapBuilder::new()
        .build(CanonPath::canonicalize(&root).map_err(anyhow::Error::msg)?)?;

    let compiled = CompiledProgram::new_with_dep(
        source,
        &dependencies,
        &UnstableFeatures::all(),
        arguments,
        false,
        Box::new(ElementsJetHinter::new()),
    )
    .map_err(anyhow::Error::msg)?;

    let satisfied = compiled.satisfy(witness).map_err(anyhow::Error::msg)?;
    let env = dummy_env::dummy();

    BitMachine::for_program(satisfied.redeem())?.exec(satisfied.redeem(), &env)?;

    let pruned = satisfied.redeem().prune(&env)?;
    let (program_bytes, witness_bytes) = pruned.to_vec_with_witness();
    println!(
        "Program: {} bytes; witness: {} bytes; witness={witness_bytes:02x?}",
        program_bytes.len(),
        witness_bytes.len()
    );

    let decoded = RedeemNode::decode::<_, _, Elements>(
        BitIter::from(program_bytes.as_slice()),
        BitIter::from(witness_bytes.as_slice()),
    )?;

    assert_eq!(decoded.cmr(), pruned.cmr());
    Ok(())
}

// Succeeds (the inactive branch was removed)
#[test]
fn single_none_without_inactive_branch_roundtrips() -> anyhow::Result<()> {
    let source =
        "fn main() { let first: Option<u256> = witness::FIRST; assert!(is_none::<u256>(first)); }";

    let arguments = Arguments::default();
    let compiled = CompiledProgram::new(
        source,
        arguments.clone(),
        false,
        Box::new(ElementsJetHinter::new()),
    )
    .map_err(anyhow::Error::msg)?;
    let witness = WitnessValues::from_map(HashMap::from([(
        TemplateProgramWitness::witness_from_str("FIRST"),
        Value::none(ResolvedType::u256()),
    )]));

    println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

    let satisfied = compiled.satisfy(witness).map_err(anyhow::Error::msg)?;
    let pruned = satisfied.redeem().prune(&dummy_env::dummy())?;
    let (program_bytes, witness_bytes) = pruned.to_vec_with_witness();

    RedeemNode::decode::<_, _, Elements>(
        BitIter::from(program_bytes.as_slice()),
        BitIter::from(witness_bytes.as_slice()),
    )?;
    Ok(())
}
