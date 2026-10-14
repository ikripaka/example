use std::{path::Path, sync::Arc};

use anyhow::Context;
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

// Close contract to the original version
#[test]
fn assert_original_none_256_encoding_roundtrip() -> anyhow::Result<()> {
    let arguments = Arguments::default();

    let entries = [
        ("FUNCTION_INDEX", Value::u8(14)),
        ("FIRST_ARG_U1", Value::some(Value::u1(0))),
        ("SECOND_ARG_U1", Value::some(Value::u1(0))),
        ("FIRST_ARG_U8", Value::some(Value::u8(0))),
        ("SECOND_ARG_U8", Value::some(Value::u8(0))),
        ("FIRST_ARG_U16", Value::some(Value::u16(0))),
        ("SECOND_ARG_U16", Value::some(Value::u16(0))),
        ("FIRST_ARG_U32", Value::some(Value::u32(0))),
        ("SECOND_ARG_U32", Value::some(Value::u32(0))),
        ("FIRST_ARG_U64", Value::some(Value::u64(0))),
        ("SECOND_ARG_U64", Value::some(Value::u64(0))),
        ("FIRST_ARG_U256", Value::none(ResolvedType::u256())),
        ("SECOND_ARG_U256", Value::some(Value::u256(U256::from(0u8)))),
    ];
    let witness = WitnessValues::from_map(
        entries
            .into_iter()
            .map(|(name, value)| (TemplateProgramWitness::witness_from_str(name), value))
            .collect(),
    );
    println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("simf");
    let path = root.join("asssert_original.simf");
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
    for data in pruned.as_ref().post_order_iter::<MaxSharing<Redeem>>() {
        if let Inner::Witness(value) = data.node.inner() {
            println!(
                "witness #{}: target={}, compact_bits={}",
                data.index,
                data.node.arrow().target,
                value.iter_compact().count()
            );
        }
    }
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
