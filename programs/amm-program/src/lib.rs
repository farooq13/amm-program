pub mod constants;
pub mod error;
pub mod instructions;
mod math;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("2AqmsNKx6LeQGBvZgcxwLmMUsT3n8Wp1jDaqCYiws6B1");

#[program]
pub mod amm_program {
    use super::*;

    pub fn initialize(
        ctx: Context<Initialize>,
        seed: u64,
        fee: u16,
        protocol_fee_bps: u16,
        authority: Option<Pubkey>,
    ) -> Result<()> {
        ctx.accounts
            .init(seed, fee, protocol_fee_bps, authority, ctx.bumps)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64, max_x: u64, max_y: u64) -> Result<()> {
        ctx.accounts.deposit(amount, max_x, max_y)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64, min_x: u64, min_y: u64) -> Result<()> {
        ctx.accounts.withdraw(amount, min_x, min_y)
    }

    pub fn swap(
        ctx: Context<Swap>,
        amount_in: u64,
        minimum_amount_out: u64,
        x_to_y: bool,
    ) -> Result<()> {
        ctx.accounts.swap(amount_in, minimum_amount_out, x_to_y)
    }

    pub fn claim_fees(ctx: Context<ClaimFees>) -> Result<()> {
        ctx.accounts.claim_fees()
    }
}
