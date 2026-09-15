//! Flagged Accounts Registry — a minimal Soroban contract.
//!
//! This is the one piece of StellarTrace that genuinely belongs on-chain:
//! a public, low-cardinality registry of account addresses that have been
//! confirmed suspicious by human investigators, so that *any* Soroban
//! contract or off-chain service in the ecosystem can cheaply check
//! `is_flagged(address)` without needing access to StellarTrace's private
//! investigation database.
//!
//! What is deliberately NOT stored here: transaction details, AI
//! analysis, investigator identities, notes, or anything else from the
//! off-chain investigation record. Only:
//! - the flagged address,
//! - a small numeric reason code (see `ReasonCode`),
//! - the ledger timestamp the flag was set.
//!
//! The off-chain StellarTrace audit log (see `stellartrace-audit`) is the
//! source of truth for *why* in human-readable detail; this contract only
//! needs to answer "is this address currently flagged, and under which
//! broad category," which is the minimum useful to a smart contract
//! guarding against interacting with a known-bad address.
//!
//! Writes are restricted to a single configured admin address (in
//! practice, the StellarTrace backend's operational key), enforced via
//! `require_auth`. Reads are public and unauthenticated.

#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReasonCode {
    Other = 0,
    LargeTransferPattern = 1,
    RepeatedTransactionAbuse = 2,
    AbnormalFrequency = 3,
    FlaggedCounterpartyInteraction = 4,
    UnusualAssetMovement = 5,
    ConfirmedFraud = 6,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct FlagRecord {
    pub reason: ReasonCode,
    pub flagged_at_ledger_timestamp: u64,
}

#[contracttype]
enum DataKey {
    Admin,
    Flag(Address),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RegistryError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    NotFlagged = 3,
}

#[contract]
pub struct FlaggedAccountsRegistry;

#[contractimpl]
impl FlaggedAccountsRegistry {
    /// One-time setup. `admin` is the only address permitted to flag or
    /// unflag accounts afterward.
    pub fn initialize(env: Env, admin: Address) -> Result<(), RegistryError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(RegistryError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Transfers admin control to a new address. Only callable by the
    /// current admin.
    pub fn set_admin(env: Env, new_admin: Address) -> Result<(), RegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &new_admin);
        Ok(())
    }

    /// Flags an account with a reason code. Only the admin (the
    /// StellarTrace backend's operational identity) may call this, and
    /// only after a human investigator has confirmed the account
    /// suspicious off-chain — this contract has no opinion on that
    /// process, it only records the outcome.
    pub fn flag(env: Env, account: Address, reason: ReasonCode) -> Result<(), RegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();

        let record = FlagRecord {
            reason,
            flagged_at_ledger_timestamp: env.ledger().timestamp(),
        };
        env.storage().persistent().set(&DataKey::Flag(account), &record);
        Ok(())
    }

    /// Removes a flag, e.g. after an investigation is reopened and
    /// resolved as a false positive.
    pub fn unflag(env: Env, account: Address) -> Result<(), RegistryError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(RegistryError::NotInitialized)?;
        admin.require_auth();

        let key = DataKey::Flag(account);
        if !env.storage().persistent().has(&key) {
            return Err(RegistryError::NotFlagged);
        }
        env.storage().persistent().remove(&key);
        Ok(())
    }

    /// Public, unauthenticated read: is this account currently flagged?
    pub fn is_flagged(env: Env, account: Address) -> bool {
        env.storage().persistent().has(&DataKey::Flag(account))
    }

    /// Public read of the full flag record, if present.
    pub fn get_flag(env: Env, account: Address) -> Option<FlagRecord> {
        env.storage().persistent().get(&DataKey::Flag(account))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn admin_can_flag_and_unflag() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, FlaggedAccountsRegistry);
        let client = FlaggedAccountsRegistryClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let suspicious = Address::generate(&env);

        client.initialize(&admin);
        assert!(!client.is_flagged(&suspicious));

        client.flag(&suspicious, &ReasonCode::ConfirmedFraud);
        assert!(client.is_flagged(&suspicious));

        let record = client.get_flag(&suspicious).unwrap();
        assert_eq!(record.reason, ReasonCode::ConfirmedFraud);

        client.unflag(&suspicious);
        assert!(!client.is_flagged(&suspicious));
    }

    #[test]
    fn double_initialize_fails() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, FlaggedAccountsRegistry);
        let client = FlaggedAccountsRegistryClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        client.initialize(&admin);
        let result = client.try_initialize(&admin);
        assert!(result.is_err());
    }
}
