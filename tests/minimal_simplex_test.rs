use simplex::transaction::{
    FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature,
};
use simplex_example_2::artifacts::minimal_pruning::{
    MinimalPruningProgram,
    derived_minimal_pruning::{MinimalPruningArguments, MinimalPruningWitness},
};

// Minimal contract test (Fails)
#[simplex::test]
fn minimal_pruning_none_happy_path(context: simplex::TestContext) -> anyhow::Result<()> {
    let arguments = MinimalPruningArguments {};
    let program = MinimalPruningProgram::new(arguments.clone());
    let witness = MinimalPruningWitness {
        check_none: true,
        first: None,
        second: Some([0; 32]),
    };
    let signer = context.get_default_signer();
    let script = program.get_script_pubkey(context.get_network());

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
