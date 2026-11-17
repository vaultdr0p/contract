#![cfg(test)]
//! Tests against the built wasm, deployed through the host's real deploy path.
//!
//! `env.register` approves a constructor's `require_auth` automatically, so it
//! cannot show that the constructor demands the admin's signature. These tests
//! upload the wasm that `stellar contract build` produced and deploy it from a
//! small factory contract, which runs the constructor under the environment's
//! normal authorization rules.
//!
//! They are `#[ignore]`d because the wasm must exist first. Run them with:
//!
//! ```sh
//! stellar contract build
//! cargo test --all-features -- --ignored
//! ```

extern crate std;

use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, AuthorizedFunction, Ledger, MockAuth, MockAuthInvoke},
    Address, Bytes, BytesN, ContractExecutable, Env, IntoVal, Symbol, Val, Vec,
};

use crate::DisbursementVaultClient;

const WASM: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/wasm32v1-none/release/disbursement_vault.wasm"
);

const START: u64 = 1_000_000;
const DEADLINE: u64 = START + 30 * 86_400;

/// Deploys the vault wasm from its own address, so deploying needs no
/// authorization from the factory's caller. The only authorization left is
/// whatever the vault's constructor requires.
#[contract]
pub struct Factory;

#[contractimpl]
impl Factory {
    pub fn deploy(
        env: Env,
        wasm_hash: BytesN<32>,
        admin: Address,
        token: Address,
        deadline: u64,
    ) -> Address {
        env.deployer()
            .with_current_contract(BytesN::from_array(&env, &[0; 32]))
            .deploy_contract(
                ContractExecutable::Wasm(wasm_hash),
                (admin, token, deadline),
            )
    }
}

struct Setup<'a> {
    env: Env,
    factory: FactoryClient<'a>,
    wasm_hash: BytesN<32>,
    admin: Address,
    token: Address,
}

fn setup<'a>() -> Setup<'a> {
    let wasm = std::fs::read(WASM)
        .unwrap_or_else(|e| panic!("{WASM}: {e}. Run `stellar contract build` first."));

    let env = Env::default();
    env.ledger().set_timestamp(START);
    let wasm_hash = env
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(&env, &wasm));
    let admin = Address::generate(&env);
    let token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let factory = FactoryClient::new(&env, &env.register(Factory, ()));

    Setup {
        env,
        factory,
        wasm_hash,
        admin,
        token,
    }
}

/// The address the factory deploys the vault to.
fn vault_address(s: &Setup) -> Address {
    s.env
        .deployer()
        .with_address(
            s.factory.address.clone(),
            BytesN::from_array(&s.env, &[0; 32]),
        )
        .deployed_address()
}

/// Deploying the real wasm needs the admin's own signature on the constructor.
/// No signature, or someone else's, fails. Those failed attempts leave nothing
/// behind: the admin-signed deploy then succeeds at the same address.
#[test]
#[ignore = "needs the built wasm: run `stellar contract build`, then `cargo test -- --ignored`"]
fn wasm_constructor_requires_the_admins_auth() {
    let s = setup();
    let vault = vault_address(&s);
    let alice = Address::generate(&s.env);
    let args: Vec<Val> = (s.admin.clone(), s.token.clone(), DEADLINE).into_val(&s.env);
    let signed_by = |signer: &Address| {
        s.env.mock_auths(&[MockAuth {
            address: signer,
            invoke: &MockAuthInvoke {
                contract: &vault,
                fn_name: "__constructor",
                args: Vec::clone(&args),
                sub_invokes: &[],
            },
        }]);
        s.factory
            .try_deploy(&s.wasm_hash, &s.admin, &s.token, &DEADLINE)
    };

    s.env.set_auths(&[]);
    assert!(
        s.factory
            .try_deploy(&s.wasm_hash, &s.admin, &s.token, &DEADLINE)
            .is_err(),
        "deploy must fail with no signature"
    );

    assert!(
        signed_by(&alice).is_err(),
        "another account's signature must not authorize the constructor"
    );

    assert_eq!(signed_by(&s.admin), Ok(Ok(vault.clone())));

    let deployed = DisbursementVaultClient::new(&s.env, &vault);
    assert_eq!(deployed.get_admin(), s.admin);
    assert_eq!(deployed.get_token(), s.token);
    assert_eq!(deployed.get_deadline(), DEADLINE);
}

/// The admin is the address recorded as authorizing `__constructor`.
#[test]
#[ignore = "needs the built wasm: run `stellar contract build`, then `cargo test -- --ignored`"]
fn wasm_constructor_records_the_admin_as_the_authorizer() {
    let s = setup();

    // The constructor runs inside `Factory::deploy`, so its `require_auth` is
    // a non-root authorization.
    s.env.mock_all_auths_allowing_non_root_auth();
    let vault = s
        .factory
        .deploy(&s.wasm_hash, &s.admin, &s.token, &DEADLINE);

    let auths = s.env.auths();
    let found = auths.iter().any(|(address, invocation)| {
        *address == s.admin
            && matches!(
                &invocation.function,
                AuthorizedFunction::Contract((contract, fn_name, _))
                    if *contract == vault
                        && *fn_name == Symbol::new(&s.env, "__constructor")
            )
    });
    assert!(
        found,
        "expected the admin to authorize __constructor, got {auths:?}"
    );
}
