use anchor_lang::prelude::*;

use crate::{constants::BPS_DENOMINATOR, error::AmmError};

pub(crate) fn liquidity_deposit_amounts(
    reserve_x: u64,
    reserve_y: u64,
    lp_supply: u64,
    lp_amount: u64,
) -> Result<(u64, u64)> {
    require_neq!(lp_amount, 0, AmmError::InvalidAmount);
    require!(
        reserve_x > 0 && reserve_y > 0 && lp_supply > 0,
        AmmError::NoLiquidityInPool
    );

    Ok((
        mul_div_ceil(reserve_x, lp_amount, lp_supply)?,
        mul_div_ceil(reserve_y, lp_amount, lp_supply)?,
    ))
}

pub(crate) fn liquidity_withdraw_amounts(
    reserve_x: u64,
    reserve_y: u64,
    lp_supply: u64,
    lp_amount: u64,
) -> Result<(u64, u64)> {
    require_neq!(lp_amount, 0, AmmError::InvalidAmount);
    require!(
        reserve_x > 0 && reserve_y > 0 && lp_supply > 0,
        AmmError::NoLiquidityInPool
    );
    require!(lp_amount <= lp_supply, AmmError::InsufficientBalance);

    Ok((
        mul_div_floor(reserve_x, lp_amount, lp_supply)?,
        mul_div_floor(reserve_y, lp_amount, lp_supply)?,
    ))
}

pub(crate) fn fee_amount_ceil(amount: u64, fee_bps: u16) -> Result<u64> {
    require!(u64::from(fee_bps) <= BPS_DENOMINATOR, AmmError::InvalidFee);
    mul_div_ceil(amount, u64::from(fee_bps), BPS_DENOMINATOR)
}

pub(crate) fn swap_output_amount(
    reserve_in: u64,
    reserve_out: u64,
    amount_in_after_fee: u64,
) -> Result<u64> {
    require!(
        reserve_in > 0 && reserve_out > 0 && amount_in_after_fee > 0,
        AmmError::NoLiquidityInPool
    );

    let numerator = u128::from(reserve_out)
        .checked_mul(u128::from(amount_in_after_fee))
        .ok_or(AmmError::Overflow)?;
    let denominator = u128::from(reserve_in)
        .checked_add(u128::from(amount_in_after_fee))
        .ok_or(AmmError::Overflow)?;
    let output = numerator
        .checked_div(denominator)
        .ok_or(AmmError::Overflow)?;

    u64::try_from(output).map_err(|_| error!(AmmError::Overflow))
}

fn mul_div_ceil(a: u64, b: u64, denominator: u64) -> Result<u64> {
    require_neq!(denominator, 0, AmmError::InvalidAmount);

    let numerator = u128::from(a)
        .checked_mul(u128::from(b))
        .ok_or(AmmError::Overflow)?;
    let divisor = u128::from(denominator);
    let quotient = numerator.checked_div(divisor).ok_or(AmmError::Overflow)?;
    let rounded = if numerator % divisor == 0 {
        quotient
    } else {
        quotient.checked_add(1).ok_or(AmmError::Overflow)?
    };

    u64::try_from(rounded).map_err(|_| error!(AmmError::Overflow))
}

fn mul_div_floor(a: u64, b: u64, denominator: u64) -> Result<u64> {
    require_neq!(denominator, 0, AmmError::InvalidAmount);

    let numerator = u128::from(a)
        .checked_mul(u128::from(b))
        .ok_or(AmmError::Overflow)?;
    let quotient = numerator
        .checked_div(u128::from(denominator))
        .ok_or(AmmError::Overflow)?;

    u64::try_from(quotient).map_err(|_| error!(AmmError::Overflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proportional_deposits_round_up() {
        assert_eq!(liquidity_deposit_amounts(3, 5, 2, 1).unwrap(), (2, 3));
    }

    #[test]
    fn proportional_withdrawals_round_down() {
        assert_eq!(liquidity_withdraw_amounts(3, 5, 2, 1).unwrap(), (1, 2));
    }

    #[test]
    fn fee_rounds_up_and_swap_output_rounds_down() {
        assert_eq!(fee_amount_ceil(1, 30).unwrap(), 1);
        assert_eq!(swap_output_amount(100, 200, 10).unwrap(), 18);
    }

    #[test]
    fn rejects_empty_reserves_and_zero_supply() {
        assert!(liquidity_deposit_amounts(0, 5, 2, 1).is_err());
        assert!(liquidity_deposit_amounts(5, 5, 0, 1).is_err());
    }
}
