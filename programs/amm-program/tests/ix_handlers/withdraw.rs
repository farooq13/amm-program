use {
    anchor_lang::{
        solana_program::instruction::Instruction, system_program::ID as SYSTEM_PROGRAM_ID,
        InstructionData, ToAccountMetas,
    },
    anchor_spl::associated_token::{self, ID as ASSOCIATED_TOKEN_PROGRAM_ID},
    litesvm_token::spl_token::ID as TOKEN_PROGRAM_ID,
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

pub fn create_withdraw_ix(
    user: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    mint_lp: Pubkey,
    config: Pubkey,
    vault_x: Pubkey,
    vault_y: Pubkey,
    amount: u64,
    min_x: u64,
    min_y: u64,
) -> Instruction {
    let user_key = user.pubkey();
    let user_x = associated_token::get_associated_token_address(&user_key, &mint_x);
    let user_y = associated_token::get_associated_token_address(&user_key, &mint_y);
    let user_lp = associated_token::get_associated_token_address(&user_key, &mint_lp);

    Instruction::new_with_bytes(
        amm_program::id(),
        &amm_program::instruction::Withdraw {
            amount,
            min_x,
            min_y,
        }
        .data(),
        amm_program::accounts::Withdraw {
            user: user_key,
            mint_x,
            mint_y,
            config,
            mint_lp,
            vault_x,
            vault_y,
            user_x,
            user_y,
            user_lp,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: SYSTEM_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
