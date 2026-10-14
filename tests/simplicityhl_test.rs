use std::{collections::HashMap, path::Path, sync::Arc};

use simplicityhl::{
    Arguments, CompiledProgram, ResolvedType, TemplateProgramWitness, UnstableFeatures, Value,
    WitnessNameToValueMap, WitnessValues,
    ast::ElementsJetHinter,
    dummy_env,
    num::U256,
    resolution::DependencyMapBuilder,
    simplicity::{
        BitIter, BitMachine, RedeemNode,
        dag::{DagLike, MaxSharing},
        jet::Elements,
        node::{Inner, Redeem},
    },
    source::{CanonPath, CanonSourceFile},
    types::TypeConstructible,
    value::ValueConstructible,
};

// Shortened contract close to the original ones (fail)
#[test]
fn assert_none_256_manual_fail() -> anyhow::Result<()> {
    let arguments = Arguments::from_map(HashMap::new());

    let entries = [
        ("FUNCTION_INDEX", Value::u8(14)),
        ("FIRST_ARG_U256", Value::none(ResolvedType::u256())),
        (
            "SECOND_ARG_U256",
            Value::some(Value::u256(U256::from_byte_array([0; 32]))),
        ),
    ];
    let witness = WitnessValues::from_map(
        entries
            .into_iter()
            .map(|(name, value)| (TemplateProgramWitness::witness_from_str(name), value))
            .collect(),
    );
    println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("simf");
    let source_path = root.join("asserts_bin.simf");
    let source = CanonSourceFile::new(
        CanonPath::canonicalize(&source_path).map_err(anyhow::Error::msg)?,
        Arc::from(std::fs::read_to_string(&source_path)?),
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
    let mut machine = BitMachine::for_program(satisfied.redeem())?;
    machine.exec(satisfied.redeem(), &env)?;

    let pruned = satisfied.redeem().prune(&env)?;
    for data in pruned.as_ref().post_order_iter::<MaxSharing<Redeem>>() {
        if let Inner::Witness(value) = data.node.inner() {
            println!(
                "witness #{}: target={}, compact_bits={}",
                data.index,
                data.node.arrow().target,
                value.iter_compact().count(),
            );
        }
    }

    let (program_bytes, witness_bytes) = pruned.to_vec_with_witness();
    println!(
        "Encoded program: {} bytes; encoded witness: {} bytes",
        program_bytes.len(),
        witness_bytes.len(),
    );
    println!("Encoded witness: {witness_bytes:02x?}");

    let decoded = RedeemNode::decode::<_, _, Elements>(
        BitIter::from(program_bytes.as_slice()),
        BitIter::from(witness_bytes.as_slice()),
    )?;

    assert_eq!(decoded.cmr(), pruned.cmr());
    Ok(())
}
