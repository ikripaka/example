use simplex::transaction::{
    FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature,
};
use simplex_example_2::artifacts::asssert_original::{
    AsssertOriginalProgram,
    derived_asssert_original::{AsssertOriginalArguments, AsssertOriginalWitness},
};

#[simplex::test]
fn assert_original_none_256_happy_path(context: simplex::TestContext) -> anyhow::Result<()> {
    let arguments = AsssertOriginalArguments {};
    let program = AsssertOriginalProgram::new(AsssertOriginalArguments {});
    let witness = AsssertOriginalWitness {
        function_index: 14,
        first_arg_u1: Some(0),
        second_arg_u1: Some(0),
        first_arg_u8: Some(0),
        second_arg_u8: Some(0),
        first_arg_u16: Some(0),
        second_arg_u16: Some(0),
        first_arg_u32: Some(0),
        second_arg_u32: Some(0),
        first_arg_u64: Some(0),
        second_arg_u64: Some(0),
        first_arg_u256: None,
        second_arg_u256: Some([0; 32]),
    };
    println!("Arguments: {arguments:?}; Witness: `{witness:?}`");

    let signer = context.get_default_signer();
    let script = program.get_script_pubkey(context.get_network());
    signer.send(script.clone(), 50_000)?.wait()?;
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
