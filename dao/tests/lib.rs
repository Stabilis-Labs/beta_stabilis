use dao::dao::dao_test::*;
use scrypto_test::prelude::*;

use scrypto_test::prelude::*;

pub fn two_resource_environment<F>(func: F)
where
    F: FnOnce(TestEnvironment, Bucket, Bucket),
{
    let mut env = TestEnvironment::new();
    let bucket1 = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(dec!("100000000000"), &mut env)
        .unwrap();
    let bucket2 = ResourceBuilder::new_fungible(OwnerRole::None)
        .mint_initial_supply(dec!("100000000000"), &mut env)
        .unwrap();

    func(env, bucket1, bucket2)

    /* Potential teardown happens here */
}

#[test]
fn contribution_provides_expected_amount_of_pool_units() {
    two_resource_environment(|mut env, bucket1, bucket2| { /* Your test goes here */ })
}
