use {
    anchor_lang::AccountDeserialize,
    anchor_spl::associated_token,
    anchor_spl::token::TokenAccount,
    litesvm::LiteSVM,
    litesvm_token::CreateMint,
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

// Setup function to initialize LiteSVM and create a payer keypair
fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = amm_program::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!(concat!(
        env!("CARGO_TARGET_TMPDIR"),
        "/../deploy/amm_program.so"
    ));

    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create two mints (Mint A and Mint B) with 6 decimal places and the maker as the authority
    // This done using litesvm-token's CreateMint utility which creates the mint in the LiteSVM environment
    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config =
        Pubkey::find_program_address(&[b"config", &123u64.to_le_bytes()], &amm_program::id()).0;
    let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_program::id()).0;

    // Derive the PDA for the vault associated token account using the config PDA and Mint A
    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);

    (
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    )
}

fn token_amount(svm: &LiteSVM, address: Pubkey) -> u64 {
    let account = svm.get_account(&address).expect("token account exists");
    let mut data = account.data.as_slice();
    TokenAccount::try_deserialize(&mut data)
        .expect("token account data is valid")
        .amount
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();

    let instruction = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    assert!(svm.get_account(&config).is_some());
}

#[test]
pub fn test_deposit() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
}

#[test]
fn test_swap_collects_and_claims_protocol_fee() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let treasury_authority =
        Pubkey::find_program_address(&[b"treasury", config.as_ref()], &amm_program::id()).0;
    let treasury_x = associated_token::get_associated_token_address(&treasury_authority, &mint_x);
    let treasury_y = associated_token::get_associated_token_address(&treasury_authority, &mint_y);
    let user_x = associated_token::get_associated_token_address(&payer.pubkey(), &mint_x);
    let user_y = associated_token::get_associated_token_address(&payer.pubkey(), &mint_y);

    let swap_ix = create_swap_ix(
        &payer,
        mint_x,
        mint_y,
        config,
        vault_x,
        vault_y,
        100_000_000,
        60_000_000,
        true,
    );
    let res = send(&mut svm, &[swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(token_amount(&svm, treasury_x), 50_000);
    assert_eq!(token_amount(&svm, treasury_y), 0);

    let reverse_swap_ix = create_swap_ix(
        &payer,
        mint_x,
        mint_y,
        config,
        vault_x,
        vault_y,
        100_000_000,
        100_000_000,
        false,
    );
    let res = send(&mut svm, &[reverse_swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(token_amount(&svm, treasury_y), 50_000);

    let user_x_before_claim = token_amount(&svm, user_x);
    let user_y_before_claim = token_amount(&svm, user_y);
    let claim_ix = create_claim_fees_ix(&payer, mint_x, mint_y, config);
    let res = send(&mut svm, &[claim_ix], &payer, &[&payer]);
    assert!(res.is_ok());
    assert_eq!(token_amount(&svm, treasury_x), 0);
    assert_eq!(token_amount(&svm, treasury_y), 0);
    assert_eq!(token_amount(&svm, user_x), user_x_before_claim + 50_000);
    assert_eq!(token_amount(&svm, user_y), user_y_before_claim + 50_000);
}
