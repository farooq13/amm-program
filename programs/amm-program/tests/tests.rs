use {
    anchor_lang::AccountDeserialize,
    anchor_spl::associated_token,
    anchor_spl::token::TokenAccount,
    litesvm::LiteSVM,
    litesvm_token::{CreateAssociatedTokenAccount, CreateMint},
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

fn assert_error(result: litesvm::types::TransactionResult, code: &str) {
    let error = result.expect_err("transaction should fail");
    let expected = format!("Error Code: {code}");
    assert!(
        error.meta.logs.iter().any(|log| log.contains(&expected)),
        "{error:?}"
    );
}

fn ensure_ata(svm: &mut LiteSVM, payer: &Keypair, owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let address = associated_token::get_associated_token_address(owner, mint);
    if svm.get_account(&address).is_none() {
        let created = CreateAssociatedTokenAccount::new(svm, payer, mint)
            .owner(owner)
            .send()
            .unwrap();
        assert_eq!(created, address);
    }
    address
}

fn create_treasury_accounts(
    svm: &mut LiteSVM,
    payer: &Keypair,
    config: Pubkey,
    mint_x: Pubkey,
    mint_y: Pubkey,
) {
    let authority =
        Pubkey::find_program_address(&[b"treasury", config.as_ref()], &amm_program::id()).0;
    ensure_ata(svm, payer, &authority, &mint_x);
    ensure_ata(svm, payer, &authority, &mint_y);
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

    let account = svm.get_account(&config).expect("config was initialized");
    let mut data = account.data.as_slice();
    let config_state = amm_program::state::Config::try_deserialize(&mut data).unwrap();
    assert_eq!(config_state.seed, 123);
    assert_eq!(config_state.fee, 30);
    assert_eq!(config_state.protocol_fee_bps, 0);
    assert_eq!(config_state.mint_x, mint_x);
    assert_eq!(config_state.mint_y, mint_y);
}

#[test]
fn test_initialize_rejects_invalid_fee() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let instruction = create_initialise_ix_with_params(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, 10_000, 0, None,
    );

    assert_error(
        send(&mut svm, &[instruction], &payer, &[&payer]),
        "InvalidFee",
    );
}

#[test]
fn test_initialize_rejects_protocol_fee_without_authority() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let instruction = create_initialise_ix_with_params(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, 30, 5, None,
    );

    assert_error(
        send(&mut svm, &[instruction], &payer, &[&payer]),
        "NoAuthoritySet",
    );
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

    let res = send(&mut svm, &[init_ix, deposit_ix.clone()], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let res = send(&mut svm, &[deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
}

#[test]
fn test_deposit_rejects_zero_amount() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix_with_params(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        0,
        200_000_000,
        200_000_000,
    );

    assert_error(
        send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]),
        "InvalidAmount",
    );
}

#[test]
fn test_deposit_rejects_excessive_slippage() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let deposit_ix = create_deposit_ix_with_params(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        100_000_000,
        199_999_999,
        200_000_000,
    );
    assert_error(
        send(&mut svm, &[deposit_ix], &payer, &[&payer]),
        "SlippageLimitExceeded",
    );
}

#[test]
fn test_withdraw_burns_lp_and_returns_proportional_assets() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let user_x = associated_token::get_associated_token_address(&payer.pubkey(), &mint_x);
    let user_y = associated_token::get_associated_token_address(&payer.pubkey(), &mint_y);
    let user_lp = associated_token::get_associated_token_address(&payer.pubkey(), &mint_lp);

    let withdraw_ix = create_withdraw_ix(
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        50_000_000,
        100_000_000,
        100_000_000,
    );
    let res = send(&mut svm, &[withdraw_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(token_amount(&svm, vault_x), 100_000_000);
    assert_eq!(token_amount(&svm, vault_y), 100_000_000);
    assert_eq!(token_amount(&svm, user_x), 900_000_000);
    assert_eq!(token_amount(&svm, user_y), 900_000_000);
    assert_eq!(token_amount(&svm, user_lp), 50_000_000);

    let withdraw_ix = create_withdraw_ix(
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        50_000_000,
        100_000_000,
        100_000_000,
    );
    let res = send(&mut svm, &[withdraw_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    assert_eq!(token_amount(&svm, vault_x), 0);
    assert_eq!(token_amount(&svm, vault_y), 0);
    assert_eq!(token_amount(&svm, user_x), 1_000_000_000);
    assert_eq!(token_amount(&svm, user_y), 1_000_000_000);
    assert_eq!(token_amount(&svm, user_lp), 0);
}

#[test]
fn test_withdraw_rejects_zero_amount() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let withdraw_ix = create_withdraw_ix(
        &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y, 0, 0, 0,
    );
    assert_error(
        send(&mut svm, &[withdraw_ix], &payer, &[&payer]),
        "InvalidAmount",
    );
}

#[test]
fn test_withdraw_rejects_excessive_amount_and_slippage() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let excessive_amount_ix = create_withdraw_ix(
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        100_000_001,
        0,
        0,
    );
    assert_error(
        send(&mut svm, &[excessive_amount_ix], &payer, &[&payer]),
        "InsufficientBalance",
    );

    let excessive_slippage_ix = create_withdraw_ix(
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        50_000_000,
        100_000_001,
        100_000_000,
    );
    assert_error(
        send(&mut svm, &[excessive_slippage_ix], &payer, &[&payer]),
        "SlippageLimitExceeded",
    );
}

#[test]
fn test_swap_collects_and_claims_protocol_fee() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix_with_params(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        30,
        5,
        Some(payer.pubkey()),
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    create_treasury_accounts(&mut svm, &payer, config, mint_x, mint_y);
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

#[test]
fn test_swap_rejects_zero_input_and_excessive_slippage() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    create_treasury_accounts(&mut svm, &payer, config, mint_x, mint_y);

    let zero_input_ix =
        create_swap_ix(&payer, mint_x, mint_y, config, vault_x, vault_y, 0, 0, true);
    assert_error(
        send(&mut svm, &[zero_input_ix], &payer, &[&payer]),
        "InvalidAmount",
    );

    let excessive_slippage_ix = create_swap_ix(
        &payer,
        mint_x,
        mint_y,
        config,
        vault_x,
        vault_y,
        100_000_000,
        u64::MAX,
        true,
    );
    assert_error(
        send(&mut svm, &[excessive_slippage_ix], &payer, &[&payer]),
        "SlippageLimitExceeded",
    );
}

#[test]
fn test_claim_fees_rejects_missing_authority() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix(
        &mut svm, &payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y,
    );
    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");
    create_treasury_accounts(&mut svm, &payer, config, mint_x, mint_y);
    ensure_ata(&mut svm, &payer, &payer.pubkey(), &mint_x);
    ensure_ata(&mut svm, &payer, &payer.pubkey(), &mint_y);

    let claim_ix = create_claim_fees_ix(&payer, mint_x, mint_y, config);
    assert_error(
        send(&mut svm, &[claim_ix], &payer, &[&payer]),
        "NoAuthoritySet",
    );
}

#[test]
fn test_claim_fees_rejects_unauthorized_signer() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y) = setup();
    let init_ix = create_initialise_ix_with_params(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        30,
        5,
        Some(payer.pubkey()),
    );
    let res = send(&mut svm, &[init_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "{res:?}");

    let unauthorized = Keypair::new();
    create_treasury_accounts(&mut svm, &payer, config, mint_x, mint_y);
    ensure_ata(&mut svm, &payer, &unauthorized.pubkey(), &mint_x);
    ensure_ata(&mut svm, &payer, &unauthorized.pubkey(), &mint_y);

    let claim_ix = create_claim_fees_ix(&unauthorized, mint_x, mint_y, config);
    assert_error(
        send(&mut svm, &[claim_ix], &payer, &[&payer, &unauthorized]),
        "InvalidAuthority",
    );
}
