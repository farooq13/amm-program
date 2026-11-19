use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::ID as ASSOCIATED_TOKEN_PROGRAM_ID,
    litesvm::LiteSVM,
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

pub fn create_initialise_ix(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    config: Pubkey,
    mint_lp: Pubkey,
    vault_x: Pubkey,
    vault_y: Pubkey,
) -> Instruction {
    create_initialise_ix_with_params(
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, 30, 0, None,
    )
}

pub fn create_initialise_ix_with_params(
    _svm: &mut LiteSVM,
    payer: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    config: Pubkey,
    mint_lp: Pubkey,
    vault_x: Pubkey,
    vault_y: Pubkey,
    fee: u16,
    protocol_fee_bps: u16,
    authority: Option<Pubkey>,
) -> Instruction {
    let maker = payer.pubkey();
    Instruction::new_with_bytes(
        amm_program::id(),
        &amm_program::instruction::Initialize {
            seed: 123,
            fee,
            protocol_fee_bps,
            authority,
        }
        .data(),
        amm_program::accounts::Initialize {
            initializer: maker,
            mint_x,
            mint_y,
            mint_lp,
            vault_x,
            vault_y,
            config,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
