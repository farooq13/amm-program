use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{burn, transfer, Burn, Mint, Token, TokenAccount, Transfer},
};

use crate::{error::AmmError, math, state::Config};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint_x: Box<Account<'info, Mint>>,
    pub mint_y: Box<Account<'info, Mint>>,
    #[account(
        has_one = mint_x,
        has_one = mint_y,
        seeds = [b"config", config.seed.to_le_bytes().as_ref()],
        bump = config.config_bump,
    )]
    pub config: Box<Account<'info, Config>>,
    #[account(
        mut,
        seeds = [b"lp", config.key().as_ref()],
        bump = config.lp_bump,
        mint::authority = config,
    )]
    pub mint_lp: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = mint_x,
        associated_token::authority = config,
    )]
    pub vault_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = config,
    )]
    pub vault_y: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_x,
        associated_token::authority = user,
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_lp,
        associated_token::authority = user,
    )]
    pub user_lp: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn withdraw(&self, amount: u64, min_x: u64, min_y: u64) -> Result<()> {
        require_neq!(amount, 0, AmmError::InvalidAmount);
        require!(amount <= self.user_lp.amount, AmmError::InsufficientBalance);

        let (amount_x, amount_y) = math::liquidity_withdraw_amounts(
            self.vault_x.amount,
            self.vault_y.amount,
            self.mint_lp.supply,
            amount,
        )?;
        require!(
            amount_x >= min_x && amount_y >= min_y,
            AmmError::SlippageLimitExceeded
        );
        require!(amount_x > 0 && amount_y > 0, AmmError::InvalidAmount);

        let burn_accounts = Burn {
            mint: self.mint_lp.to_account_info(),
            from: self.user_lp.to_account_info(),
            authority: self.user.to_account_info(),
        };
        burn(
            CpiContext::new(self.token_program.key(), burn_accounts),
            amount,
        )?;

        let config_seed = self.config.seed.to_le_bytes();
        let config_bump = [self.config.config_bump];
        let signer_seeds: &[&[&[u8]]] = &[&[b"config", config_seed.as_ref(), config_bump.as_ref()]];
        self.transfer_tokens(true, amount_x, signer_seeds)?;
        self.transfer_tokens(false, amount_y, signer_seeds)
    }

    fn transfer_tokens(&self, is_x: bool, amount: u64, signer_seeds: &[&[&[u8]]]) -> Result<()> {
        let (from, to) = if is_x {
            (
                self.vault_x.to_account_info(),
                self.user_x.to_account_info(),
            )
        } else {
            (
                self.vault_y.to_account_info(),
                self.user_y.to_account_info(),
            )
        };

        let transfer_accounts = Transfer {
            from,
            to,
            authority: self.config.to_account_info(),
        };
        transfer(
            CpiContext::new_with_signer(self.token_program.key(), transfer_accounts, signer_seeds),
            amount,
        )
    }
}
