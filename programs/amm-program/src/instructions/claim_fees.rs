use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{error::AmmError, state::Config};

#[derive(Accounts)]
pub struct ClaimFees<'info> {
    pub authority: Signer<'info>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Account<'info, Config>,
    #[account(
        seeds = [b"treasury", config.key().as_ref()],
        bump = config.treasury_bump,
    )]
    /// CHECK: This PDA is only used as the authority for the treasury token accounts.
    pub treasury_authority: UncheckedAccount<'info>,
    #[account(address = config.mint_x)]
    pub mint_x: Account<'info, Mint>,
    #[account(address = config.mint_y)]
    pub mint_y: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = treasury_authority,
    )]
    pub treasury_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = treasury_authority,
    )]
    pub treasury_y: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = authority,
    )]
    pub authority_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = authority,
    )]
    pub authority_y: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

impl<'info> ClaimFees<'info> {
    pub fn claim_fees(&self) -> Result<()> {
        require!(self.config.authority.is_some(), AmmError::NoAuthoritySet);
        require!(
            self.config.authority == Some(self.authority.key()),
            AmmError::InvalidAuthority
        );

        let config_key = self.config.key();
        let treasury_bump = [self.config.treasury_bump];
        let signer_seeds: &[&[&[u8]]] =
            &[&[b"treasury", config_key.as_ref(), treasury_bump.as_ref()]];
        let token_program = self.token_program.key();

        if self.treasury_x.amount > 0 {
            let transfer_accounts = Transfer {
                from: self.treasury_x.to_account_info(),
                to: self.authority_x.to_account_info(),
                authority: self.treasury_authority.to_account_info(),
            };
            transfer(
                CpiContext::new_with_signer(token_program, transfer_accounts, signer_seeds),
                self.treasury_x.amount,
            )?;
        }

        if self.treasury_y.amount > 0 {
            let transfer_accounts = Transfer {
                from: self.treasury_y.to_account_info(),
                to: self.authority_y.to_account_info(),
                authority: self.treasury_authority.to_account_info(),
            };
            transfer(
                CpiContext::new_with_signer(token_program, transfer_accounts, signer_seeds),
                self.treasury_y.amount,
            )?;
        }

        Ok(())
    }
}
