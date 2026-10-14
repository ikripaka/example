use simplex::program::ProgramTrait;
use simplex::provider::SimplicityNetwork;
use simplex::simplicityhl::elements::{
    TxOut, confidential,
    pset::{Input, PartiallySignedTransaction},
};
use simplex::simplicityhl::simplicity::{
    BitIter, RedeemNode,
    dag::{DagLike, MaxSharing},
    jet::Elements,
    node::{Inner, Redeem},
};
use simplex::transaction::{
    FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature,
};
use simplex_example_2::artifacts::asserts_bin::{
    AssertsBinProgram,
    derived_asserts_bin::{AssertsBinArguments, AssertsBinWitness},
};

fn assert_none_256_witness() -> AssertsBinWitness {
    AssertsBinWitness {
        function_index: 14,
        first_arg_u256: None,
        second_arg_u256: Some([0; 32]),
    }
}

#[test]
fn assert_none_256_witness_encoding_roundtrip() -> anyhow::Result<()> {
    let arguments = AssertsBinArguments {};
    let program = AssertsBinProgram::new(arguments.clone());
    let witness = assert_none_256_witness();
    let network = SimplicityNetwork::default_regtest();

    let mut pst = PartiallySignedTransaction::new_v2();
    pst.add_input(Input {
        witness_utxo: Some(TxOut {
            asset: confidential::Asset::Explicit(network.policy_asset()),
            value: confidential::Value::Explicit(50),
            script_pubkey: program.get_script_pubkey(&network),
            ..Default::default()
        }),
        ..Default::default()
    });

    let (pruned, _) = program
        .as_ref()
        .execute(&pst, &witness.into(), 0, &network)?;

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

#[simplex::test]
fn assert_none_256_happy_path(context: simplex::TestContext) -> anyhow::Result<()> {
    let arguments = AssertsBinArguments {};
    let program = AssertsBinProgram::new(arguments.clone());
    let witness = assert_none_256_witness();
    let signer = context.get_default_signer();
    let script = program.get_script_pubkey(context.get_network());

    println!(
        "Arguments: {:?}; Witness: {:?}",
        arguments.clone().to_owned(),
        witness
    );

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
