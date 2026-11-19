use {
    anchor_lang::{solana_program::instruction::Instruction, InstructionData, ToAccountMetas},
    anchor_spl::{associated_token, token::ID as TOKEN_PROGRAM_ID},
    solana_keypair::Keypair,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
};

pub fn create_claim_fees_ix(
    authority: &Keypair,
    mint_x: Pubkey,
    mint_y: Pubkey,
    config: Pubkey,
) -> Instruction {
    let authority_key = authority.pubkey();
    let treasury_authority =
        Pubkey::find_program_address(&[b"treasury", config.as_ref()], &amm_program::id()).0;
    let treasury_x = associated_token::get_associated_token_address(&treasury_authority, &mint_x);
    let treasury_y = associated_token::get_associated_token_address(&treasury_authority, &mint_y);
    let authority_x = associated_token::get_associated_token_address(&authority_key, &mint_x);
    let authority_y = associated_token::get_associated_token_address(&authority_key, &mint_y);

    Instruction::new_with_bytes(
        amm_program::id(),
        &amm_program::instruction::ClaimFees {}.data(),
        amm_program::accounts::ClaimFees {
            authority: authority_key,
            config,
            treasury_authority,
            mint_x,
            mint_y,
            treasury_x,
            treasury_y,
            authority_x,
            authority_y,
            token_program: TOKEN_PROGRAM_ID,
        }
        .to_account_metas(None),
    )
}
