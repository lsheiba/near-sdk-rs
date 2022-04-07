use borsh::BorshSchema;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Error, Write};

use crate::{AccountId, Balance, Gas, PromiseIndex, PublicKey};

enum PromiseSubtype {
    Single(PromiseIndex),
    Joint(PromiseIndex),
}

impl PromiseSubtype {
    fn index(&self) -> PromiseIndex {
        match self {
            Self::Single(x) => *x,
            Self::Joint(x) => *x,
        }
    }
}

/// A structure representing a result of the scheduled execution on another contract.
///
/// Smart contract developers will explicitly use `Promise` in two situations:
/// * When they need to return `Promise`.
///
///   In the following code if someone calls method `ContractA::a` they will internally cause an
///   execution of method `ContractB::b` of `bob_near` account, and the return value of `ContractA::a`
///   will be what `ContractB::b` returned.
/// ```no_run
/// # use near_sdk::{ext_contract, near_bindgen, Promise, Gas};
/// # use borsh::{BorshDeserialize, BorshSerialize};
/// #[ext_contract]
/// pub trait ContractB {
///     fn b(&mut self);
/// }
///
/// #[near_bindgen]
/// #[derive(Default, BorshDeserialize, BorshSerialize)]
/// struct ContractA {}
///
/// #[near_bindgen]
/// impl ContractA {
///     pub fn a(&self) -> Promise {
///         contract_b::b("bob_near".parse().unwrap(), 0, Gas(1_000))
///     }
/// }
/// ```
///
/// * When they need to create a transaction with one or many actions, e.g. the following code
///   schedules a transaction that creates an account, transfers tokens, and assigns a public key:
///
/// ```no_run
/// # use near_sdk::{Promise, env, test_utils::VMContextBuilder, testing_env};
/// # testing_env!(VMContextBuilder::new().signer_account_id("bob_near".parse().unwrap())
/// #               .account_balance(1000).prepaid_gas(1_000_000.into()).build());
/// Promise::new("bob_near".parse().unwrap())
///   .create_account()
///   .transfer(1000)
///   .add_full_access_key(env::signer_account_pk());
/// ```
pub struct Promise {
    index: PromiseSubtype,
    should_return: RefCell<bool>,
}

/// Until we implement strongly typed promises we serialize them as unit struct.
impl BorshSchema for Promise {
    fn add_definitions_recursively(
        definitions: &mut HashMap<borsh::schema::Declaration, borsh::schema::Definition>,
    ) {
        <()>::add_definitions_recursively(definitions);
    }

    fn declaration() -> borsh::schema::Declaration {
        <()>::declaration()
    }
}

impl Promise {
    /// Create a promise that acts on the given account.
    pub fn new(account_id: &AccountId) -> Self {
        Self {
            index: PromiseSubtype::Single(crate::env::promise_batch_create(&account_id)),
            should_return: RefCell::new(false),
        }
    }

    // TODO this should prob be restricted at compile time
    fn action_index(&self) -> PromiseIndex {
        match self.index {
            PromiseSubtype::Single(x) => x,
            PromiseSubtype::Joint(_) => crate::env::panic_str("Cannot add action to a joint promise."),
        }
    }

    // fn add_action(self, action: PromiseAction) -> Self {
    //     match &self.index {
    //         PromiseTy::Single(x) => x.actions.borrow_mut().push(action),
    //         PromiseSubtype::Joint(_) => {
    //             crate::env::panic_str("Cannot add action to a joint promise.")
    //         }
    //     }
    //     self
    // }

    /// Create account on which this promise acts.
    pub fn create_account(self) -> Self {
        crate::env::promise_batch_action_create_account(self.action_index());
        self
    }

    /// Deploy a smart contract to the account on which this promise acts.
    pub fn deploy_contract(self, code: &[u8]) -> Self {
        crate::env::promise_batch_action_deploy_contract(self.action_index(), code);
        self
    }

    /// A low-level interface for making a function call to the account that this promise acts on.
    pub fn function_call(
        self,
        function_name: &str,
        arguments: &[u8],
        amount: Balance,
        gas: Gas,
    ) -> Self {
        crate::env::promise_batch_action_function_call(
            self.action_index(),
            function_name,
            arguments,
            amount,
            gas,
        );
        self
    }

    /// Transfer tokens to the account that this promise acts on.
    pub fn transfer(self, amount: Balance) -> Self {
        crate::env::promise_batch_action_transfer(self.action_index(), amount);
        self
    }

    /// Stake the account for the given amount of tokens using the given public key.
    pub fn stake(self, amount: Balance, public_key: &PublicKey) -> Self {
        crate::env::promise_batch_action_stake(self.action_index(), amount, public_key);
        self
    }

    /// Add full access key to the given account.
    pub fn add_full_access_key(self, public_key: &PublicKey) -> Self {
        crate::env::promise_batch_action_add_key_with_full_access(
            self.action_index(),
            public_key,
            0,
        );
        self
    }

    /// Add full access key to the given account with a provided nonce.
    pub fn add_full_access_key_with_nonce(self, public_key: &PublicKey, nonce: u64) -> Self {
        crate::env::promise_batch_action_add_key_with_full_access(
            self.action_index(),
            public_key,
            nonce,
        );
        self
    }

    /// Add an access key that is restricted to only calling a smart contract on some account using
    /// only a restricted set of methods. Here `function_names` is a comma separated list of methods,
    /// e.g. `"method_a,method_b".to_string()`.
    pub fn add_access_key(
        self,
        public_key: &PublicKey,
        allowance: Balance,
        receiver_id: &AccountId,
        // TODO maybe want to change this to slice of &str
        function_names: &str,
    ) -> Self {
        crate::env::promise_batch_action_add_key_with_function_call(
            self.action_index(),
            public_key,
            0,
            allowance,
            receiver_id,
            function_names,
        );
        self
    }

    /// Add an access key with a provided nonce.
    pub fn add_access_key_with_nonce(
        self,
        public_key: &PublicKey,
        allowance: Balance,
        receiver_id: &AccountId,
        function_names: &str,
        nonce: u64,
    ) -> Self {
        crate::env::promise_batch_action_add_key_with_function_call(
            self.action_index(),
            public_key,
            nonce,
            allowance,
            receiver_id,
            function_names,
        );
        self
    }

    /// Delete access key from the given account.
    pub fn delete_key(self, public_key: &PublicKey) -> Self {
        crate::env::promise_batch_action_delete_key(self.action_index(), public_key);
        self
    }

    /// Delete the given account.
    pub fn delete_account(self, beneficiary_id: &AccountId) -> Self {
        crate::env::promise_batch_action_delete_account(self.action_index(), beneficiary_id);
        self
    }

    /// Merge this promise with another promise, so that we can schedule execution of another
    /// smart contract right after all merged promises finish.
    ///
    /// Note, once the promises are merged it is not possible to add actions to them, e.g. the
    /// following code will panic during the execution of the smart contract:
    ///
    /// ```no_run
    /// # use near_sdk::{Promise, testing_env};
    /// let p1 = Promise::new("bob_near".parse().unwrap()).create_account();
    /// let p2 = Promise::new("carol_near".parse().unwrap()).create_account();
    /// let p3 = p1.and(p2);
    /// // p3.create_account();
    /// ```
    pub fn and(self, other: Promise) -> Promise {
        Self {
            // TODO current impl seems to call unnecessary `promise_and`. Yes, this might be
            // TODO functional, but more optimal if `and` all at once.
            index: PromiseSubtype::Joint(crate::env::promise_and(&[
                self.index.index(),
                other.index.index(),
            ])),
            should_return: RefCell::new(false),
        }
    }

    /// Schedules execution of another promise right after the current promise finish executing.
    ///
    /// In the following code `bob_near` and `dave_near` will be created concurrently. `carol_near`
    /// creation will wait for `bob_near` to be created, and `eva_near` will wait for both `carol_near`
    /// and `dave_near` to be created first.
    /// ```no_run
    /// # use near_sdk::{Promise, VMContext, testing_env};
    /// let p1 = Promise::new("bob_near".parse().unwrap()).create_account();
    /// let p2 = Promise::new("carol_near".parse().unwrap()).create_account();
    /// let p3 = Promise::new("dave_near".parse().unwrap()).create_account();
    /// let p4 = Promise::new("eva_near".parse().unwrap()).create_account();
    /// p1.then(p2).and(p3).then(p4);
    /// ```
    pub fn then(self, other: &AccountId) -> Promise {
        Self {
            index: PromiseSubtype::Single(crate::env::promise_batch_then(self.index.index(), other)),
            should_return: RefCell::new(false),
        }
    }

    /// A specialized, relatively low-level API method. Allows to mark the given promise as the one
    /// that should be considered as a return value.
    ///
    /// In the below code `a1` and `a2` functions are equivalent.
    /// ```
    /// # use near_sdk::{ext_contract, Gas, near_bindgen, Promise};
    /// # use borsh::{BorshDeserialize, BorshSerialize};
    /// #[ext_contract]
    /// pub trait ContractB {
    ///     fn b(&mut self);
    /// }
    ///
    /// #[near_bindgen]
    /// #[derive(Default, BorshDeserialize, BorshSerialize)]
    /// struct ContractA {}
    ///
    /// #[near_bindgen]
    /// impl ContractA {
    ///     pub fn a1(&self) {
    ///        contract_b::b("bob_near".parse().unwrap(), 0, Gas(1_000)).as_return();
    ///     }
    ///
    ///     pub fn a2(&self) -> Promise {
    ///        contract_b::b("bob_near".parse().unwrap(), 0, Gas(1_000))
    ///     }
    /// }
    /// ```
    #[allow(clippy::wrong_self_convention)]
    pub fn as_return(self) -> Self {
        *self.should_return.borrow_mut() = true;
        self
    }
}

impl serde::Serialize for Promise {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        *self.should_return.borrow_mut() = true;
        serializer.serialize_unit()
    }
}

impl borsh::BorshSerialize for Promise {
    fn serialize<W: Write>(&self, _writer: &mut W) -> Result<(), Error> {
        *self.should_return.borrow_mut() = true;

        // Intentionally no bytes written for the promise, the return value from the promise
        // will be considered as the return value from the contract call.
        Ok(())
    }
}

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum PromiseOrValue<T> {
    Promise(Promise),
    Value(T),
}

impl<T> BorshSchema for PromiseOrValue<T>
where
    T: BorshSchema,
{
    fn add_definitions_recursively(
        definitions: &mut HashMap<borsh::schema::Declaration, borsh::schema::Definition>,
    ) {
        T::add_definitions_recursively(definitions);
    }

    fn declaration() -> borsh::schema::Declaration {
        T::declaration()
    }
}

impl<T> From<Promise> for PromiseOrValue<T> {
    fn from(promise: Promise) -> Self {
        PromiseOrValue::Promise(promise)
    }
}

impl<T: borsh::BorshSerialize> borsh::BorshSerialize for PromiseOrValue<T> {
    fn serialize<W: Write>(&self, writer: &mut W) -> Result<(), Error> {
        match self {
            // Only actual value is serialized.
            PromiseOrValue::Value(x) => x.serialize(writer),
            // The promise is dropped to cause env::promise calls.
            PromiseOrValue::Promise(p) => p.serialize(writer),
        }
    }
}
