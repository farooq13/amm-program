use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::state::Config;

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub initializer: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,

    #[account(
        init,
        payer = initializer,
        seeds = [b"config", seed.to_le_bytes().as_ref()],
        bump,
        space = Config::DISCRIMINATOR.len() + Config::INIT_SPACE,
    )]
    pub config: Account<'info, Config>,

    #[account(
        init,
        payer = initializer,
        seeds = [b"lp", config.key().as_ref()],
        bump,
        mint::decimals = 6,
        mint::authority = config,
    )]
    pub mint_lp: Account<'info, Mint>,

    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,

    #[account(
        init,
        payer = initializer,
        associated_token::mint = mint_y,
        associated_token::authority = config
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn init(
        &mut self,
        seed: u64,
        fee: u16,
        protocol_fee_bps: u16,
        authority: Option<Pubkey>,
        bumps: InitializeBumps,
    ) -> Result<()> {
        require!(
            fee < 10_000 && protocol_fee_bps <= fee,
            crate::error::AmmError::InvalidFee
        );
        require_keys_neq!(
            self.mint_x.key(),
            self.mint_y.key(),
            crate::error::AmmError::InvalidToken
        );
        require!(
            protocol_fee_bps == 0 || authority.is_some(),
            crate::error::AmmError::NoAuthoritySet
        );
        let config_key = self.config.key();
        let (_, treasury_bump) = Pubkey::find_program_address(
            &[b"treasury", config_key.as_ref()],
            &crate::ID,
        );

        self.config.set_inner(Config {
            seed,
            authority,
            mint_x: self.mint_x.key(),
            mint_y: self.mint_y.key(),
            fee,
            protocol_fee_bps,
            locked: false,
            config_bump: bumps.config,
            lp_bump: bumps.mint_lp,
            treasury_bump,
        });

        Ok(())
    }
}
