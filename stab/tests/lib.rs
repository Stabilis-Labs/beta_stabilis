use scrypto_test::prelude::*;
use stab_module::shared_structs::CdpStatus;
use stab_module::stabilis_component::stabilis_component_test::*;

// Generic setup
pub fn publish_and_setup()-> Result<(TestEnvironment<InMemorySubstateDatabase>, Stabilis, Bucket, Bucket), RuntimeError>
{
    let mut env = TestEnvironment::new();
    env.disable_auth_module();
    let package = PackageFactory::compile_and_publish(this_package!(), &mut env, CompileProfile::Fast)?;

    let (mut stab_comp, controller_badge) = Stabilis::instantiate(
        package,
        &mut env
    )?;

    let a_bucket = ResourceBuilder::new_fungible(OwnerRole::None) 
        .divisibility(18)
        .mint_initial_supply(10000, &mut env)?;

    stab_comp.add_collateral( 
        a_bucket.resource_address(&mut env)?,
        dec!("1.5"),
        dec!("1"),
        &mut env,
    )?;

    assert_eq!(controller_badge.amount(&mut env)?, dec!(10));

    Ok((env, stab_comp, a_bucket, controller_badge))
}

// Individual tests
#[test]
fn deploys() -> Result<(), RuntimeError> {
    let (_env, _stab_comp, _a_bucket, _control_bucket) = publish_and_setup()?;
    Ok(())
}

// Can open CDP
#[test]
fn can_open_cdp() -> Result<(Bucket, Bucket), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp( 
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    assert_eq!(stab.amount(&mut env)?, dec!(500));

    Ok((stab, cdp))
}

// Fail to open CDP with insufficient collateral
#[test]
fn fail_open_cdp_insufficient_collateral() {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    // Attempt to open CDP with insufficient collateral
    let result = stab_comp.open_cdp(
        a_bucket.take(dec!(10), &mut env)?,  // Only 10 units of collateral
        dec!(500),  // Trying to mint 500 STAB
        &mut env,
    );

    assert!(result.is_err());
}

// Can close CDP
#[test]
fn can_close_cdp() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    // Repay the loan and close the CDP
    let (collateral, leftover_stab) = stab_comp.close_cdp(cdp.clone(), stab, &mut env)?;
    assert_eq!(collateral.amount(&mut env)?, dec!(1000));
    assert_eq!(leftover_stab.amount(&mut env)?, dec!(0));

    Ok(())
}

// Can partial close CDP
#[test]
fn can_partial_close_cdp() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    // Repay the loan and close the CDP
    stab_comp.partial_close_cdp(cdp.clone(), stab.take(dec!(100), &mut env)?, &mut env)?;

    assert_eq!(stab.amount(&mut env)?, dec!(400));

    Ok(())
}

// Cant close CDP with too little repayment
#[test]
fn cant_close_cdp_insufficient_repayment() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    // Repay the loan and close the CDP
    let result = stab_comp.close_cdp(cdp.clone(), stab.take(dec!(400), &mut env)?, &mut env);

    assert!(result.is_err());

    Ok(())
}

// Cant partial close below minimum mint
#[test]
fn cant_partial_close_cdp_below_minimum_mint() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    // Repay the loan and close the CDP
    let result = stab_comp.partial_close_cdp(cdp.clone(), stab.take(dec!("499.5"), &mut env)?, &mut env);

    assert!(result.is_err());

    Ok(())
}

// Cant close CDP with wrong repayment resource
#[test]
fn cant_close_cdp_wrong_resource() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (_stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    // Repay the loan and close the CDP
    let result = stab_comp.close_cdp(cdp.clone(), a_bucket.take(dec!(500), &mut env)?, &mut env);

    assert!(result.is_err());

    Ok(())
}

// Cant close CDP with wrong repayment resource
#[test]
fn cant_partial_close_cdp_wrong_resource() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (_stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    // Repay the loan and close the CDP
    let result = stab_comp.partial_close_cdp(cdp.clone(), a_bucket.take(dec!(500), &mut env)?, &mut env);

    assert!(result.is_err());

    Ok(())
}

// Top up CDP works
#[test]
fn can_top_up_cdp() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (_stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    stab_comp.top_up_cdp(cdp.clone(), a_bucket.take(dec!(500), &mut env)?, &mut env);

    assert_eq!(a_bucket.amount(&mut env)?, dec!(8500));

    Ok(())
}

// Top up CDP doesn't work with wrong resource
#[test]
fn cant_top_up_cdp_wrong_payment() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    let result = stab_comp.top_up_cdp(cdp.clone(), stab.take(dec!(500), &mut env)?, &mut env);

    assert!(result.is_err());

    Ok(())
}

// Top up CDP works, and removes updated amount of collateral after
#[test]
fn can_top_up_cdp_and_remove() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    stab_comp.top_up_cdp(cdp.clone(), a_bucket.take(dec!(500), &mut env)?, &mut env);

    let (collateral, leftover_stab) = stab_comp.close_cdp(cdp.clone(), stab, &mut env)?;

    assert_eq!(collateral.amount(&mut env), dec!(1500));
    assert_eq!(leftover_stab.amount(&mut env), dec!(0));

    Ok(())
}

// Removing collateral works
#[test]
fn can_remove_collateral() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (_stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    let removed_collateral = stab_comp.remove_collateral(cdp.clone(), dec!(100), &mut env)?;

    assert_eq!(removed_collateral.amount(&mut env)?, dec!(100));
    assert_eq!(removed_collateral.resource_address(&mut env)?, a_bucket.resource_address(&mut env)?);

    Ok(())
}

// Can't remove too much collateral
#[test]
fn cant_remove_collateral_below_mcr() -> Result<(), RuntimeError> {
    let (mut env, mut stab_comp, a_bucket, _control_bucket) = publish_and_setup()?;

    let (_stab, cdp) = stab_comp.open_cdp(
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        &mut env,
    )?;

    let cdps = cdp.non_fungible_local_ids(&mut env)?;
    let cdp = cdps.first().unwrap();

    let result = stab_comp.remove_collateral(cdp.clone(), dec!(400), &mut env)?;

    assert!(result.is_err());

    Ok(())
}