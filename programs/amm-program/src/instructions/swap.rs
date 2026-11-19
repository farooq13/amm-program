use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};

use crate::{constants::BPS_DENOMINATOR, error::AmmError, state::Config};

#[derive(Accounts)]
pub struct Swap<'info> {
    pub user: Signer<'info>,
    pub mint_x: Account<'info, Mint>,
    pub mint_y: Account<'info, Mint>,
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
        associated_token::authority = user,
    )]
    pub user_x: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        associated_token::mint = mint_y,
        associated_token::authority = user,
    )]
    pub user_y: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
}

impl<'info> Swap<'info> {
    pub fn swap(&mut self, amount_in: u64, minimum_amount_out: u64, x_to_y: bool) -> Result<()> {
        require!(!self.config.locked, AmmError::PoolLocked);
        require_neq!(amount_in, 0, AmmError::InvalidAmount);

        let (user_input, user_output, pool_input_vault, pool_output_vault, treasury_input) =
            if x_to_y {
                (
                    &self.user_x,
                    &self.user_y,
                    &self.vault_x,
                    &self.vault_y,
                    &self.treasury_x,
                )
            } else {
                (
                    &self.user_y,
                    &self.user_x,
                    &self.vault_y,
                    &self.vault_x,
                    &self.treasury_y,
                )
            };
        let (reserve_in, reserve_out) = (pool_input_vault.amount, pool_output_vault.amount);
        require!(
            reserve_in > 0 && reserve_out > 0,
            AmmError::NoLiquidityInPool
        );

        let total_fee = calculate_fee_rounded_up(amount_in, self.config.fee)?;
        let protocol_fee = calculate_fee_rounded_up(amount_in, self.config.protocol_fee_bps)?;
        require!(protocol_fee <= total_fee, AmmError::InvalidFee);
        let effective_input = amount_in
            .checked_sub(total_fee)
            .ok_or(AmmError::Underflow)?;
        require!(effective_input > 0, AmmError::InvalidAmount);

        let output_numerator = u128::from(reserve_out)
            .checked_mul(u128::from(effective_input))
            .ok_or(AmmError::Overflow)?;
        let output_denominator = u128::from(reserve_in)
            .checked_add(u128::from(effective_input))
            .ok_or(AmmError::Overflow)?;
        let amount_out = u64::try_from(
            output_numerator
                .checked_div(output_denominator)
                .ok_or(AmmError::Overflow)?,
        )
        .map_err(|_| error!(AmmError::Overflow))?;
        require!(
            amount_out >= minimum_amount_out,
            AmmError::SlippageLimitExceeded
        );
        require_neq!(amount_out, 0, AmmError::InvalidAmount);

        let pool_input = amount_in
            .checked_sub(protocol_fee)
            .ok_or(AmmError::Underflow)?;
        require!(pool_input > 0, AmmError::InvalidAmount);

        pool_input_vault
            .amount
            .checked_add(pool_input)
            .ok_or(AmmError::Overflow)?;
        treasury_input
            .amount
            .checked_add(protocol_fee)
            .ok_or(AmmError::Overflow)?;
        user_output
            .amount
            .checked_add(amount_out)
            .ok_or(AmmError::Overflow)?;

        let token_program = self.token_program.key();
        let user_input_transfer = Transfer {
            from: user_input.to_account_info(),
            to: pool_input_vault.to_account_info(),
            authority: self.user.to_account_info(),
        };
        transfer(
            CpiContext::new(token_program, user_input_transfer),
            pool_input,
        )?;

        if protocol_fee > 0 {
            let treasury_transfer = Transfer {
                from: user_input.to_account_info(),
                to: treasury_input.to_account_info(),
                authority: self.user.to_account_info(),
            };
            transfer(
                CpiContext::new(token_program, treasury_transfer),
                protocol_fee,
            )?;
        }

        let config_seed = self.config.seed.to_le_bytes();
        let config_bump = [self.config.config_bump];
        let signer_seeds: &[&[&[u8]]] = &[&[b"config", config_seed.as_ref(), config_bump.as_ref()]];
        let pool_output_transfer = Transfer {
            from: pool_output_vault.to_account_info(),
            to: user_output.to_account_info(),
            authority: self.config.to_account_info(),
        };
        transfer(
            CpiContext::new_with_signer(token_program, pool_output_transfer, signer_seeds),
            amount_out,
        )
    }
}

fn calculate_fee_rounded_up(amount: u64, fee_bps: u16) -> Result<u64> {
    let numerator = u128::from(amount)
        .checked_mul(u128::from(fee_bps))
        .ok_or(AmmError::Overflow)?;
    let rounded_fee = numerator
        .checked_add(u128::from(BPS_DENOMINATOR - 1))
        .ok_or(AmmError::Overflow)?
        .checked_div(u128::from(BPS_DENOMINATOR))
        .ok_or(AmmError::Overflow)?;

    u64::try_from(rounded_fee).map_err(|_| error!(AmmError::Overflow))
}
