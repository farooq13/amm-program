use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    anchor_spl::{associated_token, token::ID as TOKEN_PROGRAM_ID},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

pub fn create_swap_ix(
    user: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    config: Pubkey,
    vault_x: Pubkey,
    vault_y: Pubkey,
    amount_in: u64,
    minimum_amount_out: u64,
    x_to_y: bool,
) -> Instruction {
    let user_key = user.pubkey();
    let treasury_authority =
        Pubkey::find_program_address(&[b"treasury", config.as_ref()], &amm_program::id()).0;
    let treasury_x = associated_token::get_associated_token_address(&treasury_authority, &mint_x);
    let treasury_y = associated_token::get_associated_token_address(&treasury_authority, &mint_y);
    let user_x = associated_token::get_associated_token_address(&user_key, &mint_x);
    let user_y = associated_token::get_associated_token_address(&user_key, &mint_y);

    Instruction::new_with_bytes(
        amm_program::id(),
        &amm_program::instruction::Swap {
            amount_in,
            minimum_amount_out,
            x_to_y,
        }
        .data(),
        amm_program::accounts::Swap {
            user: user_key,
            mint_x,
            mint_y,
            config,
            treasury_authority,
            vault_x,
            vault_y,
            treasury_x,
            treasury_y,
            user_x,
            user_y,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
