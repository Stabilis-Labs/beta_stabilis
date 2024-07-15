use scrypto_test::prelude::*;

use stab_module::stab_module::*;

// Generic setup
pub fn publish_and_setup<F>(func: F) -> Result<(), RuntimeError>
   where
    F: FnOnce(TestEnvironment, &mut Stabilis, Bucket) -> Result<(), RuntimeError> 
{
    let mut env = TestEnvironment::new();
    let package = Package::compile_and_publish(this_package!(), &mut env)?;

    let mut stab_comp = Stabilis::instantiate(
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
    )?;

    stab_comp.open_cdp( 
        a_bucket.take(dec!(1000), &mut env)?,
        dec!(500),
        true,
    )?;

    Ok(func(env, &mut stab_comp, a_bucket)?)
}

// Individual tests
#[test]
fn deploys() -> Result<(), RuntimeError> {
    publish_and_setup(|mut _env, &mut _stab_comp, _a_bucket| -> Result<(), RuntimeError> {
        Ok(())
    })
}